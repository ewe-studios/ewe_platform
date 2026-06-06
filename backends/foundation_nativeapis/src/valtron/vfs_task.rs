//! VfsTask — valtron task that consumes ObservableFs events via Broadcaster.
//!
//! # Overview
//!
//! `VfsTask` bridges [`ObservableFs`](crate::vfs::ObservableFs) with the valtron
//! task execution model. It subscribes to the broadcaster's event stream and
//! yields events through the valtron iterator infrastructure.
//!
//! # Example
//!
//! ```ignore
//! use foundation_nativeapis::vfs::{ObservableFs, MemoryFs};
//! use foundation_nativeapis::valtron::VfsTask;
//! use foundation_core::valtron::execute;
//!
//! let mut fs = ObservableFs::new(MemoryFs::new());
//! let mut task = VfsTask::from_observable(&mut fs);
//! let stop = task.stop_signal();
//! let stream = execute(task, None)?;
//!
//! // Consume events with collect_one or iterate manually
//! ```

use std::sync::Arc;

use foundation_core::synca::mpp::{Receiver, ReceiverError};
use foundation_core::valtron::{BoxedSendExecutionAction, DrivenStreamIterator, EventReadiness, TaskIterator, TaskStatus};
use foundation_core::valtron::{execute, GenericResult};

use crate::shared::vfs::{ObservableFs, VfsEvent, VfsFileSystem};

use super::broadcaster::Broadcaster as EventBroadcaster;
use super::stop_signal::CompositeReadiness;
use super::StopSignal;

/// Readiness indicator for VFS events.
///
/// Checks whether the mpp receiver channel has pending items.
pub struct VfsEventReadiness {
    receiver: Receiver<VfsEvent>,
}

impl VfsEventReadiness {
    /// Create a new readiness from a receiver.
    #[must_use]
    pub fn new(receiver: Receiver<VfsEvent>) -> Self {
        Self { receiver }
    }
}

impl EventReadiness for VfsEventReadiness {
    fn is_ready(&self, _dur: Option<std::time::Duration>) -> bool {
        !self.receiver.is_empty()
    }
}

/// A valtron task that consumes [`ObservableFs`] events and yields them
/// through the valtron executor model.
///
/// On each `next_status()` call:
/// 1. Tries to receive an event from the mpp channel (non-blocking).
/// 2. If available: broadcasts to downstream subscribers, returns `Ready(event)`.
/// 3. If empty: returns `Depends(readiness)` to park the task.
pub struct VfsTask {
    receiver: Receiver<VfsEvent>,
    broadcaster: EventBroadcaster<VfsEvent>,
    stop: StopSignal,
}

impl VfsTask {
    /// Subscribe to an [`ObservableFs`] event stream.
    ///
    /// # Panics
    ///
    /// Never panics. The subscribe call is on a `Mutex<Broadcaster>` which
    /// will only panic if the mutex is poisoned (unrecoverable).
    pub fn from_observable<F: VfsFileSystem>(fs: &mut ObservableFs<F>) -> Self {
        let receiver = fs.subscribe();
        Self {
            receiver,
            broadcaster: EventBroadcaster::new(64),
            stop: StopSignal::new(),
        }
    }

    /// Set the bounded channel capacity for downstream subscribers.
    ///
    /// Higher capacity = more memory, fewer drops under load.
    /// Lower capacity = less memory, more drops under load.
    /// Default is 64.
    #[must_use]
    pub fn with_channel_capacity(mut self, cap: usize) -> Self {
        self.broadcaster = EventBroadcaster::new(cap);
        self
    }

    /// Get a `StopSignal` that can terminate this task when signaled.
    #[must_use]
    pub fn stop_signal(&self) -> StopSignal {
        self.stop.clone()
    }

    /// Subscribe to VFS events re-broadcast by this task.
    pub fn subscribe(&mut self) -> Receiver<VfsEvent> {
        self.broadcaster.subscribe()
    }

    /// Get the number of downstream subscribers.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.broadcaster.subscriber_count()
    }

    fn make_depends(&self) -> TaskStatus<VfsEvent, (), BoxedSendExecutionAction> {
        TaskStatus::Depends(Arc::new(CompositeReadiness::new(
            Arc::new(VfsEventReadiness::new(self.receiver.clone())),
            Arc::new(self.stop.clone()),
        )))
    }
}

impl TaskIterator for VfsTask {
    type Ready = VfsEvent;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    #[tracing::instrument(skip(self), fields(stop = self.stop.is_stopped()))]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.stop.is_stopped() {
            return None;
        }

        match self.receiver.recv() {
            Ok(event) => {
                self.broadcaster.broadcast(event.clone());
                Some(TaskStatus::Ready(event))
            }
            Err(ReceiverError::Empty) => Some(self.make_depends()),
            Err(ReceiverError::Closed(_)) => None,
            Err(ReceiverError::Timeout) => Some(self.make_depends()),
        }
    }
}

/// Builder for creating a `VfsTask` and spawning it into valtron.
///
/// # Example
///
/// ```ignore
/// use foundation_nativeapis::vfs::{ObservableFs, MemoryFs};
/// use foundation_nativeapis::valtron::VfsTaskBuilder;
/// use foundation_core::valtron::collect_one;
///
/// let mut fs = ObservableFs::new(MemoryFs::new());
/// let stream = VfsTaskBuilder::from_observable(&mut fs).build()?;
///
/// if let Some(event) = collect_one(stream) {
///     println!("VFS event: {:?}", event);
/// }
/// ```
pub struct VfsTaskBuilder {
    task: VfsTask,
}

impl VfsTaskBuilder {
    /// Create a builder from an [`ObservableFs`].
    pub fn from_observable<F: VfsFileSystem>(fs: &mut ObservableFs<F>) -> Self {
        Self {
            task: VfsTask::from_observable(fs),
        }
    }

    /// Spawn the task into the valtron executor and return the stream.
    pub fn build(self) -> GenericResult<DrivenStreamIterator<VfsTask>> {
        execute(self.task, None)
    }
}
