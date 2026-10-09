//! Background workers — typed channels + session constructors (F34).
//!
//! WHY: Mutation queue replay, cache warming, and sync need to run when the
//! app isn't rendering. The platform provides typed channels and session
//! spawn methods — no proc macros. Users wire workers in `setup_routes()`.
//!
//! WHAT: `WorkerChannel<T>` wraps `concurrent_queue::ConcurrentQueue` into a
//! named, cloneable, typed command sender. `WorkerRegistry` lets callers
//! dispatch commands by name. The session provides `spawn_worker()`.
//!
//! HOW: Workers are closures that receive a receiver and an `Arc<Session>`.
//! `try_recv()` is non-blocking — workers spin or await externally. The
//! registry dispatches commands by name through type-erased senders.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use concurrent_queue::ConcurrentQueue;

use crate::session::PlatformSession;

// ── WorkerChannel ───────────────────────────────────────────────────────

/// Typed, bounded, named channel for worker commands. Built via builder
/// pattern, then the receiver is consumed by `session.spawn_worker()`.
pub struct WorkerChannel<T: Send + 'static> {
    name: String,
    queue: Arc<ConcurrentQueue<T>>,
}

impl<T: Send + 'static> Clone for WorkerChannel<T> {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), queue: Arc::clone(&self.queue) }
    }
}

impl<T: Send + 'static> WorkerChannel<T> {
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), queue: Arc::new(ConcurrentQueue::bounded(64)) }
    }

    /// Set capacity (default 64). Must be called before `build()`.
    #[must_use]
    pub fn capacity(mut self, n: usize) -> Self {
        self.queue = Arc::new(ConcurrentQueue::bounded(n.max(1)));
        self
    }

    /// Build the channel. Returns `(sender, receiver)`. The receiver half
    /// is consumed by `session.spawn_worker()`. The sender is cloneable
    /// and can be sent to the registry for cross-site dispatch.
    pub fn build(self) -> (Self, WorkerReceiver<T>) {
        let rx = WorkerReceiver { queue: Arc::clone(&self.queue) };
        (self, rx)
    }

    /// Send a command. Non-blocking — returns false if channel is full.
    pub fn send(&self, cmd: T) -> bool {
        self.queue.push(cmd).is_ok()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Raw queue reference for registry type-erased dispatch.
    fn queue_arc(&self) -> Arc<ConcurrentQueue<T>> {
        Arc::clone(&self.queue)
    }
}

// ── WorkerReceiver ──────────────────────────────────────────────────────

/// The consumer half of a `WorkerChannel`. Owned by the worker closure.
/// Call `recv()` in a loop to process commands. Returns `None` when no
/// senders remain (channel fully dropped).
pub struct WorkerReceiver<T: Send + 'static> {
    queue: Arc<ConcurrentQueue<T>>,
}

impl<T: Send + 'static> WorkerReceiver<T> {
    /// Blocking receive. Returns `None` when the channel is closed (all
    /// senders dropped AND the queue is empty).
    pub fn recv(&mut self) -> Option<T> {
        self.queue.pop().ok()
    }

    /// Non-blocking receive. Returns `None` if empty (not closed).
    pub fn try_recv(&mut self) -> Option<T> {
        match self.queue.pop() {
            Ok(item) => Some(item),
            Err(concurrent_queue::PopError::Empty) => None,
            Err(concurrent_queue::PopError::Closed) => None,
        }
    }
}

// ── WorkerRegistry ──────────────────────────────────────────────────────

trait ErasedSender: Send + Sync {
    fn try_send(&self, item: Box<dyn std::any::Any + Send>) -> bool;
}

struct TypedSender<T: Send + 'static> {
    queue: Arc<ConcurrentQueue<T>>,
}

impl<T: Send + 'static> ErasedSender for TypedSender<T> {
    fn try_send(&self, item: Box<dyn std::any::Any + Send>) -> bool {
        match item.downcast::<T>() {
            Ok(typed) => self.queue.push(*typed).is_ok(),
            Err(_) => false,
        }
    }
}

/// Thread-safe registry of named worker channels. Keyed by worker name.
/// Workers register on spawn; callers dispatch by name.
pub struct WorkerRegistry {
    channels: Mutex<HashMap<String, Box<dyn ErasedSender>>>,
}

impl WorkerRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { channels: Mutex::new(HashMap::new()) }
    }

    /// Register a channel for named dispatch.
    pub fn register<T: Send + 'static>(&self, name: &str, queue: Arc<ConcurrentQueue<T>>) {
        self.channels.lock().unwrap().insert(
            name.to_string(),
            Box::new(TypedSender { queue }),
        );
    }

    /// Send a command to a named worker. Returns `false` if unknown or
    /// the type doesn't match.
    pub fn send<T: Send + 'static>(&self, name: &str, cmd: T) -> bool {
        match self.channels.lock().unwrap().get(name) {
            Some(erased) => erased.try_send(Box::new(cmd)),
            None => false,
        }
    }

    #[must_use]
    pub fn exists(&self, name: &str) -> bool {
        self.channels.lock().unwrap().contains_key(name)
    }

    pub fn deregister(&self, name: &str) {
        self.channels.lock().unwrap().remove(name);
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.channels.lock().unwrap().keys().cloned().collect()
    }
}

impl Default for WorkerRegistry {
    fn default() -> Self { Self::new() }
}

// ── Session integration ──────────────────────────────────────────────────

impl PlatformSession {
    /// Spawn a worker on a background thread. The closure receives the
    /// receiver and an `Arc<PlatformSession>`. Runs until the channel closes
    /// (all senders dropped) or the closure returns.
    ///
    /// # Example
    /// ```ignore
    /// let (tx, rx) = WorkerChannel::<SyncCommand>::new("sync").build();
    /// session.spawn_worker(tx.clone(), rx, |mut rx, session| {
    ///     while let Some(cmd) = rx.recv() {
    ///         match cmd {
    ///             SyncCommand::FullSync => { /* ... */ }
    ///         }
    ///     }
    /// });
    /// // Dispatch from anywhere:
    /// session.workers().send("sync", SyncCommand::FullSync);
    /// ```
    pub fn spawn_worker<T, F>(self: &Arc<Self>, channel: WorkerChannel<T>, receiver: WorkerReceiver<T>, handler: F)
    where
        T: Send + 'static,
        F: FnOnce(WorkerReceiver<T>, Arc<PlatformSession>) + Send + 'static,
    {
        let name = channel.name().to_string();
        self.workers().register(&name, channel.queue_arc());

        let session = self.clone();
        std::thread::spawn(move || {
            handler(receiver, session);
        });
    }

    /// Access the worker registry to send commands by name.
    #[must_use]
    pub fn workers(&self) -> &WorkerRegistry {
        &self.workers_
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_send_and_recv() {
        let (tx, mut rx) = WorkerChannel::<String>::new("test").capacity(4).build();
        assert!(tx.send("hello".into()));
        assert!(tx.send("world".into()));
        assert_eq!(rx.recv(), Some("hello".into()));
        assert_eq!(rx.recv(), Some("world".into()));
    }

    #[test]
    fn channel_full_drops() {
        let (tx, _rx) = WorkerChannel::<i32>::new("full").capacity(2).build();
        assert!(tx.send(1));
        assert!(tx.send(2));
        assert!(!tx.send(3));
    }

    #[test]
    fn channel_name() {
        let (tx, _) = WorkerChannel::<()>::new("sync_v2").build();
        assert_eq!(tx.name(), "sync_v2");
    }

    #[test]
    fn channel_clone_dispatches_to_same_queue() {
        let (tx1, mut rx) = WorkerChannel::<i32>::new("shared").build();
        let tx2 = tx1.clone();
        tx1.send(10);
        tx2.send(20);
        drop(tx1);
        drop(tx2);
        let mut got = Vec::new();
        while let Some(v) = rx.try_recv() { got.push(v); }
        got.sort();
        assert_eq!(got, vec![10, 20]);
    }

    #[test]
    fn registry_register_and_send() {
        let reg = WorkerRegistry::new();
        let (tx, mut rx) = WorkerChannel::<String>::new("echo").build();
        reg.register("echo", tx.queue_arc());
        assert!(reg.exists("echo"));
        assert!(reg.send("echo", "ping".to_string()));
        assert_eq!(rx.recv(), Some("ping".into()));
    }

    #[test]
    fn registry_wrong_type_dropped() {
        let reg = WorkerRegistry::new();
        let (tx, _rx) = WorkerChannel::<String>::new("str").build();
        reg.register("str", tx.queue_arc());
        assert!(!reg.send("str", 42i32)); // wrong type
    }

    #[test]
    fn registry_missing_worker() {
        let reg = WorkerRegistry::new();
        assert!(!reg.send("nope", 42i32));
    }

    #[test]
    fn registry_names() {
        let reg = WorkerRegistry::new();
        let (t1, _) = WorkerChannel::<i32>::new("a").build();
        let (t2, _) = WorkerChannel::<String>::new("b").build();
        reg.register("a", t1.queue_arc());
        reg.register("b", t2.queue_arc());
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn registry_deregister() {
        let reg = WorkerRegistry::new();
        let (tx, _) = WorkerChannel::<i32>::new("tmp").build();
        reg.register("tmp", tx.queue_arc());
        assert!(reg.exists("tmp"));
        reg.deregister("tmp");
        assert!(!reg.exists("tmp"));
    }
}
