/// Valtron tasks for file watching and FD monitoring.
///
/// Requires the `task` feature flag (which depends on foundation_core for valtron types).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::mpp::{self, Receiver, Sender};
use foundation_core::valtron::{
    BoxedSendExecutionAction, TaskIterator, TaskStatus,
};

use crate::error::Result;
use crate::event::WatchEvent;
use crate::watcher::{NativeWatcher, SharedWatcher};

mod fd_monitor;
pub use fd_monitor::FdMonitorTask;

/// Multi-subscriber broadcaster built on top of mpp channels.
///
/// Each subscriber gets an independent `Receiver<T>`.
/// When `broadcast()` is called, the event is pushed to every subscriber's queue.
/// Dead subscriber channels (Receiver dropped) are cleaned up automatically.
pub struct EventBroadcaster<T: Clone + Send + 'static> {
    subscribers: Vec<Sender<T>>,
    capacity: usize,
}

impl<T: Clone + Send + 'static> EventBroadcaster<T> {
    /// Create a new broadcaster with the given per-subscriber channel capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            subscribers: Vec::new(),
            capacity,
        }
    }

    /// Subscribe — returns a (Sender<T>, Receiver<T>) pair.
    ///
    /// The Sender is also returned so the subscriber can explicitly close
    /// their own channel if needed.
    pub fn subscribe(&mut self) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpp::bounded(self.capacity);
        self.subscribers.push(tx.clone());
        (tx, rx)
    }

    /// Send an event to all subscribers.
    ///
    /// Dead subscriber channels (Receiver dropped) are cleaned up automatically.
    /// Uses `force_send` to drop oldest events if a subscriber's queue is full.
    pub fn broadcast(&mut self, event: T) {
        self.subscribers.retain(|tx| {
            // If the channel is closed (receiver dropped), try_send will fail
            match tx.send(event.clone()) {
                Ok(()) => true,
                Err(_) => {
                    // Channel is full or closed — try force_send
                    match tx.force_send(event.clone()) {
                        Ok(_dropped) => true, // Queue full, dropped oldest but still alive
                        Err(_) => false,      // Channel closed — remove this subscriber
                    }
                }
            }
        });
    }

    /// Clean up dead subscribers (Receiver dropped).
    pub fn cleanup(&mut self) {
        self.subscribers.retain(|tx| !tx.is_closed());
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

impl<T: Clone + Send + 'static> Default for EventBroadcaster<T> {
    fn default() -> Self {
        Self::new(64)
    }
}

/// A valtron task that wraps a `NativeWatcher` and delivers file events
/// to subscriber queues.
///
/// Other valtron tasks can subscribe to receive file change events through
/// their own mpp receiver channels.
///
/// **Scheduling:** `next_status()` returns `TaskStatus::Depends(shared_watcher)`
/// when no events are available. The executor uses the watcher's `EventReadiness`
/// impl to decide when to reschedule — no busy-polling with timeouts.
pub struct FileWatcherTask {
    watcher: SharedWatcher,
    broadcaster: EventBroadcaster<WatchEvent>,
}

impl FileWatcherTask {
    /// Create a new FileWatcherTask with a platform-default native watcher.
    pub fn new() -> Result<Self> {
        use crate::api::native_watcher;

        Ok(Self {
            watcher: SharedWatcher::from_boxed(native_watcher()?),
            broadcaster: EventBroadcaster::new(64),
        })
    }

    /// Create with a specific NativeWatcher implementation.
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            watcher: SharedWatcher::from_boxed(watcher),
            broadcaster: EventBroadcaster::new(64),
        }
    }

    /// Create with a pre-built shared watcher.
    pub fn with_shared_watcher(watcher: SharedWatcher) -> Self {
        Self {
            watcher,
            broadcaster: EventBroadcaster::new(64),
        }
    }

    /// Get a clone of the shared watcher handle.
    /// Use this to get an `Arc<dyn EventReadiness>` for `TaskStatus::Depends`.
    pub fn watcher(&self) -> SharedWatcher {
        self.watcher.clone_handle()
    }

    /// Add a path to watch. Returns self for chaining.
    pub fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        self.watcher.watch(path, recursive)
    }

    /// Remove a previously watched path.
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)
    }

    /// Subscribe to file events.
    ///
    /// Returns a (Sender, Receiver) pair — the Sender can be used to close
    /// the subscriber's channel explicitly.
    pub fn subscribe(&mut self) -> (Sender<WatchEvent>, Receiver<WatchEvent>) {
        self.broadcaster.subscribe()
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.broadcaster.subscriber_count()
    }
}

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Poll with zero timeout — the executor drives scheduling via Depends.
        // We only poll when the watcher says there are events to read.
        match self.watcher.poll(Duration::ZERO) {
            Ok(events) if !events.is_empty() => {
                // Broadcast all events to subscribers, return first as Ready
                let (first, rest) = events.split_first().unwrap();
                self.broadcaster.broadcast(first.clone());
                for event in rest {
                    self.broadcaster.broadcast(event.clone());
                }
                Some(TaskStatus::Ready(first.clone()))
            }
            // No events — return Depends so the executor parks this task
            // and uses EventReadiness::is_ready() to know when to reschedule.
            Ok(_) | Err(_) => {
                Some(TaskStatus::Depends(Arc::new(self.watcher.clone_handle())))
            }
        }
    }
}
