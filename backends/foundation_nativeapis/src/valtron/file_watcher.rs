/// Valtron task: FileWatcherTask — wraps a NativeWatcher and delivers file events
/// to subscriber queues.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::mpp::{Receiver, Sender};
use foundation_core::valtron::{
    BoxedSendExecutionAction, TaskIterator, TaskStatus,
};

use crate::shared::error::Result;
use crate::shared::event::WatchEvent;
use crate::shared::watcher::{NativeWatcher, SharedWatcher};

use super::broadcaster::EventBroadcaster;
use super::StopSignal;
use super::stop_signal::CompositeReadiness;

/// A valtron task that wraps a `NativeWatcher` and delivers file events
/// to subscriber queues.
///
/// Other valtron tasks can subscribe to receive file change events through
/// their own mpp receiver channels.
pub struct FileWatcherTask {
    watcher: SharedWatcher,
    broadcaster: EventBroadcaster<WatchEvent>,
    poll_timeout: Duration,
    stop: StopSignal,
}

impl FileWatcherTask {
    pub fn new() -> Result<Self> {
        use crate::shared::api::native_watcher;

        Ok(Self {
            watcher: SharedWatcher::from_boxed(native_watcher()?),
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
            stop: StopSignal::new(),
        })
    }

    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            watcher: SharedWatcher::from_boxed(watcher),
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
            stop: StopSignal::new(),
        }
    }

    pub fn with_shared_watcher(watcher: SharedWatcher) -> Self {
        Self {
            watcher,
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
            stop: StopSignal::new(),
        }
    }

    /// Get a clone of the shared watcher handle.
    pub fn watcher(&self) -> SharedWatcher {
        self.watcher.clone_handle()
    }

    /// Get a `StopSignal` that can terminate this task when signaled.
    pub fn stop_signal(&self) -> StopSignal {
        self.stop.clone()
    }

    pub fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        self.watcher.watch(path, recursive)
    }

    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)
    }

    pub fn subscribe(&mut self) -> (Sender<WatchEvent>, Receiver<WatchEvent>) {
        self.broadcaster.subscribe()
    }

    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }

    pub fn subscriber_count(&self) -> usize {
        self.broadcaster.subscriber_count()
    }
}

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    #[tracing::instrument(skip(self), fields(stop = self.stop.is_stopped()))]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Check stop signal first — if set, return None to terminate the task.
        if self.stop.is_stopped() {
            return None;
        }

        match self.watcher.poll(self.poll_timeout) {
            Ok(events) if !events.is_empty() => {
                let (first, rest) = events.split_first().unwrap();
                self.broadcaster.broadcast(first.clone());
                for event in rest {
                    self.broadcaster.broadcast(event.clone());
                }
                Some(TaskStatus::Ready(first.clone()))
            }
            // Depends on watcher OR stop signal — wakes when either has events or stop() called.
            Ok(_) | Err(_) => {
                Some(TaskStatus::Depends(Arc::new(CompositeReadiness::new(
                    Arc::new(self.watcher.clone_handle()),
                    Arc::new(self.stop.clone()),
                ))))
            }
        }
    }
}
