/// Valtron task: `FileWatcherTask` — wraps a `NativeWatcher` and delivers file events
/// to subscriber queues.
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::synca::mpp::Receiver;
use foundation_core::valtron::{execute, GenericResult};
use foundation_core::valtron::{Ctx, DrivenStreamIterator, TaskIterator, TaskStatus};

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
    /// Events from the current poll that were not yet returned.
    ///
    /// One poll can return many events. The task delivers one event per
    /// `next_status` call, so the rest wait here for later calls.
    pending: VecDeque<WatchEvent>,
}

impl FileWatcherTask {
    /// Create a task with the best native watcher for this platform.
    ///
    /// # Errors
    /// Returns a watcher error if no native watcher can be built.
    pub fn new() -> Result<Self> {
        use crate::shared::api::native_watcher;

        Ok(Self {
            watcher: SharedWatcher::from_boxed(native_watcher()?),
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
            stop: StopSignal::new(),
            pending: VecDeque::new(),
        })
    }

    #[must_use]
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            watcher: SharedWatcher::from_boxed(watcher),
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
            stop: StopSignal::new(),
            pending: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn with_shared_watcher(watcher: SharedWatcher) -> Self {
        Self {
            watcher,
            broadcaster: EventBroadcaster::new(64),
            poll_timeout: Duration::from_millis(50),
            stop: StopSignal::new(),
            pending: VecDeque::new(),
        }
    }

    /// Get a clone of the shared watcher handle.
    #[must_use]
    pub fn watcher(&self) -> SharedWatcher {
        self.watcher.clone_handle()
    }

    /// Get a `StopSignal` that can terminate this task when signaled.
    #[must_use]
    pub fn stop_signal(&self) -> StopSignal {
        self.stop.clone()
    }

    /// Add a path to watch.
    ///
    /// # Errors
    /// Returns a watcher error if the path cannot be watched.
    pub fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        self.watcher.watch(path, recursive)
    }

    /// Remove a watched path.
    ///
    /// # Errors
    /// Returns a watcher error if the path cannot be unwatched.
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        self.watcher.unwatch(path)
    }

    pub fn subscribe(&mut self) -> Receiver<WatchEvent> {
        self.broadcaster.subscribe()
    }

    #[must_use]
    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }

    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.broadcaster.subscriber_count()
    }
}

impl TaskIterator for FileWatcherTask {
    type Ready = WatchEvent;
    type Pending = ();

    #[tracing::instrument(skip(self, _ctx), fields(stop = self.stop.is_stopped()))]
    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.stop.is_stopped() {
            return None;
        }

        // Return the next event that is still waiting from an earlier poll.
        if let Some(event) = self.pending.pop_front() {
            return Some(TaskStatus::Ready(event));
        }

        match self.watcher.poll(self.poll_timeout) {
            Ok(events) if !events.is_empty() => {
                // Broadcast every event exactly once, then queue them all.
                for event in &events {
                    self.broadcaster.broadcast(event.clone());
                }
                self.pending.extend(events);
                let first = self.pending.pop_front().unwrap();
                Some(TaskStatus::Ready(first))
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
    /// Create a builder with the best native watcher for this platform.
    ///
    /// # Errors
    /// Returns a watcher error if no native watcher can be built.
    pub fn new() -> Result<Self> {
        Ok(Self {
            task: FileWatcherTask::new()?,
        })
    }

    #[must_use]
    pub fn with_watcher(watcher: Box<dyn NativeWatcher>) -> Self {
        Self {
            task: FileWatcherTask::with_watcher(watcher),
        }
    }

    #[must_use]
    pub fn with_shared_watcher(watcher: SharedWatcher) -> Self {
        Self {
            task: FileWatcherTask::with_shared_watcher(watcher),
        }
    }

    /// Add a path to watch.
    ///
    /// # Errors
    /// Returns a watcher error if the path cannot be watched.
    pub fn watch(mut self, path: impl AsRef<Path>, recursive: bool) -> Result<Self> {
        self.task.watch(path.as_ref(), recursive)?;
        Ok(self)
    }

    #[must_use]
    pub fn poll_timeout(mut self, timeout: Duration) -> Self {
        self.task = self.task.with_poll_timeout(timeout);
        self
    }

    /// Spawn the task into the valtron executor and return the stream.
    ///
    /// The caller drives consumption — use `collect_one()`, `collect_result()`,
    /// or iterate manually. The stream yields `Stream<WatchEvent, ()>` so
    /// callers can distinguish events from pending/delayed states.
    ///
    /// # Errors
    /// Returns an executor error if the task cannot be spawned.
    pub fn build(self) -> GenericResult<DrivenStreamIterator<FileWatcherTask>> {
        execute(self.task, None)
    }
}

impl Default for FileWatcherBuilder {
    fn default() -> Self {
        Self::new().expect("native_watcher() failed")
    }
}
