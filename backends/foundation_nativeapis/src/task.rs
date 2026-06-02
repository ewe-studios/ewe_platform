/// Valtron task: FileWatcherTask — wraps a NativeWatcher and broadcasts
/// file events to subscribers via mpp channels.
///
/// Each tick, polls the native watcher for events, broadcasts them to all
/// subscribers, and yields back to the valtron execution engine.

use std::path::Path;
use std::time::Duration;

use foundation_core::synca::mpp::{self, Receiver, Sender};

use crate::error::Result;
use crate::event::WatchEvent;
use crate::watcher::NativeWatcher;

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
            // Try normal send first; if full, force_push to drop oldest
            match tx.send(event.clone()) {
                Ok(()) => true,
                Err(_) => {
                    // Queue full or closed — try force_send
                    tx.force_send(event.clone()).is_ok()
                }
            }
        });
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
pub struct FileWatcherTask {
    watcher: Box<dyn NativeWatcher>,
    broadcaster: EventBroadcaster<WatchEvent>,
    poll_timeout: Duration,
}

impl FileWatcherTask {
    /// Create a new FileWatcherTask with a platform-default native watcher.
    pub fn new() -> Result<Self> {
        use crate::api::{native_watcher, WatcherBuilder};

        Ok(Self {
            watcher: native_watcher()?,
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
        })
    }

    /// Create with a specific NativeWatcher implementation.
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            watcher,
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
        }
    }

    /// Add a path to watch. Returns self for chaining.
    pub fn watch(mut self, path: &Path, recursive: bool) -> Result<Self> {
        self.watcher.watch(path, recursive)?;
        Ok(self)
    }

    /// Remove a previously watched path.
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)
    }

    /// Subscribe to file events.
    ///
    /// Returns a (Sender, Receiver) pair — the Sender can be used to close
    /// the subscriber's channel explicitly, or to send events to other subscribers.
    pub fn subscribe(&mut self) -> (Sender<WatchEvent>, Receiver<WatchEvent>) {
        self.broadcaster.subscribe()
    }

    /// Set the poll timeout for each tick. Default: 50ms.
    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.broadcaster.subscriber_count()
    }
}

impl FileWatcherTask {
    /// Poll the native watcher and broadcast events to subscribers.
    ///
    /// Returns the events that were received, or an empty Vec on timeout/error.
    /// This method is called by the valtron execution engine each tick.
    pub fn tick(&mut self) -> Vec<WatchEvent> {
        match self.watcher.poll(self.poll_timeout) {
            Ok(events) if !events.is_empty() => {
                for event in &events {
                    self.broadcaster.broadcast(event.clone());
                }
                events
            }
            Ok(_) => Vec::new(),
            Err(e) => {
                tracing::error!("Watcher poll error: {}", e);
                Vec::new()
            }
        }
    }
}
