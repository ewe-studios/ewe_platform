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

use super::broadcaster::Broadcaster as EventBroadcaster;
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

    pub fn subscribe(&mut self) -> Receiver<WatchEvent> {
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

use foundation_core::valtron::{Stream, execute, GenericResult};
use foundation_core::valtron::DrivenStreamIterator;

/// Builder for creating a `FileWatcherTask` with automatic valtron execution.
///
/// Instead of manually creating a task, subscribing, and calling `execute()`,
/// the builder handles all of that and returns a stream of `WatchEvent`.
///
/// # Example
///
/// ```ignore
/// use foundation_nativeapis::valtron::FileWatcherBuilder;
///
/// let stream = FileWatcherBuilder::new()
///     .watch("/path/to/dir", true)?
///     .build()?;
///
/// for event in stream {
///     println!("File changed: {:?}", event.path);
/// }
/// ```
pub struct FileWatcherBuilder {
    task: FileWatcherTask,
}

impl FileWatcherBuilder {
    /// Create a new builder with the default native watcher.
    pub fn new() -> Result<Self> {
        Ok(Self {
            task: FileWatcherTask::new()?,
        })
    }

    /// Create a builder with a specific watcher.
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            task: FileWatcherTask::with_watcher(watcher),
        }
    }

    /// Create a builder with a shared watcher.
    pub fn with_shared_watcher(watcher: SharedWatcher) -> Self {
        Self {
            task: FileWatcherTask::with_shared_watcher(watcher),
        }
    }

    /// Add a path to watch.
    pub fn watch(mut self, path: impl AsRef<Path>, recursive: bool) -> Result<Self> {
        self.task.watch(path.as_ref(), recursive)?;
        Ok(self)
    }

    /// Set the poll timeout.
    pub fn poll_timeout(mut self, timeout: Duration) -> Self {
        self.task = self.task.with_poll_timeout(timeout);
        self
    }

    /// Spawn the task into the valtron executor and return a stream of events.
    ///
    /// The stream yields `WatchEvent` values as files change. When the task
    /// terminates (e.g., via stop signal), the stream ends.
    pub fn build(self) -> GenericResult<WatchEventStream> {
        let stream = execute(self.task, None)?;
        Ok(WatchEventStream(stream))
    }
}

impl Default for FileWatcherBuilder {
    fn default() -> Self {
        Self::new().expect("native_watcher() failed")
    }
}

/// A stream of `WatchEvent` from a running `FileWatcherTask`.
///
/// Wraps valtron's `DrivenStreamIterator` and filters to yield only
/// `WatchEvent` values, skipping `Pending` and `Delayed` states.
pub struct WatchEventStream(DrivenStreamIterator<FileWatcherTask>);

impl Iterator for WatchEventStream {
    type Item = WatchEvent;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.0.next()? {
                Stream::Next(event) => return Some(event),
                _ => continue,
            }
        }
    }
}
