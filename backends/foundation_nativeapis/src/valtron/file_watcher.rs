/// Valtron task: FileWatcherTask — wraps a NativeWatcher and delivers file events
/// to subscriber queues.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::mpp::Receiver;
use foundation_core::valtron::{BoxedSendExecutionAction, DrivenStreamIterator, TaskIterator, TaskStatus};
use foundation_core::valtron::{execute, GenericResult};

use crate::shared::error::Result;
use crate::shared::event::WatchEvent;
use crate::shared::watcher::{NativeWatcher, SharedWatcher};

use super::broadcaster::Broadcaster as EventBroadcaster;
use super::stop_signal::CompositeReadiness;
use super::StopSignal;

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
            Ok(_) | Err(_) => Some(TaskStatus::Depends(Arc::new(CompositeReadiness::new(
                Arc::new(self.watcher.clone_handle()),
                Arc::new(self.stop.clone()),
            )))),
        }
    }
}

/// Builder for creating a `FileWatcherTask` and spawning it into valtron.
///
/// Handles task creation, watch registration, and executor scheduling.
/// Returns valtron's `DrivenStreamIterator` directly — no blocking wrapper.
///
/// # Example
///
/// ```ignore
/// use foundation_nativeapis::valtron::FileWatcherBuilder;
/// use foundation_core::valtron::collect_one;
///
/// let stream = FileWatcherBuilder::new()
///     .watch("/path/to/dir", true)?
///     .build()?;
///
/// // Non-blocking: use collect_one or collect_result from valtron
/// if let Some(event) = collect_one(stream) {
///     println!("File changed: {:?}", event.path);
/// }
/// ```
pub struct FileWatcherBuilder {
    task: FileWatcherTask,
}

impl FileWatcherBuilder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            task: FileWatcherTask::new()?,
        })
    }

    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            task: FileWatcherTask::with_watcher(watcher),
        }
    }

    pub fn with_shared_watcher(watcher: SharedWatcher) -> Self {
        Self {
            task: FileWatcherTask::with_shared_watcher(watcher),
        }
    }

    pub fn watch(mut self, path: impl AsRef<Path>, recursive: bool) -> Result<Self> {
        self.task.watch(path.as_ref(), recursive)?;
        Ok(self)
    }

    pub fn poll_timeout(mut self, timeout: Duration) -> Self {
        self.task = self.task.with_poll_timeout(timeout);
        self
    }

    /// Spawn the task into the valtron executor and return the stream.
    ///
    /// The caller drives consumption — use `collect_one()`, `collect_result()`,
    /// or iterate manually. The stream yields `Stream<WatchEvent, ()>` so
    /// callers can distinguish events from pending/delayed states.
    pub fn build(self) -> GenericResult<DrivenStreamIterator<FileWatcherTask>> {
        execute(self.task, None)
    }
}

impl Default for FileWatcherBuilder {
    fn default() -> Self {
        Self::new().expect("native_watcher() failed")
    }
}
