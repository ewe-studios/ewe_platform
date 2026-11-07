// SignalTask — valtron TaskIterator for signal reception.

use std::sync::Arc;

use foundation_core::valtron::{Ctx, EventQueue, EventReadinessPtr, TaskIterator, TaskStatus};

use crate::signal::{SignalBus, SignalEvent, SignalHandle};

/// A valtron task that receives signals from a [`SignalBus`].
///
/// The task parks on the platform handle. Each readiness poll of the handle
/// delivers the signals that the handler recorded, and the task then returns
/// them from its queue. This task is the one in-process driver of the handle,
/// so a signal is delivered only while an executor polls this task.
pub struct SignalTask {
    /// Holds the bus for the lifetime of the task, so a caller that passes its
    /// only `Arc` keeps a live subscriber list. The task reaches the bus
    /// through `my_queue`, so it never reads this field.
    _bus: Arc<SignalBus>,
    my_queue: EventQueue<SignalEvent>,
    readiness: EventReadinessPtr,
}

impl SignalTask {
    pub fn new(bus: Arc<SignalBus>, handle: SignalHandle) -> Self {
        let my_queue = bus.subscribe_events();
        let readiness: EventReadinessPtr = Arc::new(handle);
        Self {
            _bus: bus,
            my_queue,
            readiness,
        }
    }
}

impl TaskIterator for SignalTask {
    type Ready = SignalEvent;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Drain signals that a readiness poll has already delivered.
        if let Ok(event) = self.my_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }
        // Park on the platform handle. Its readiness poll delivers the signal
        // that the handler recorded, and then wakes this task through the
        // queue.
        Some(TaskStatus::Depends(Arc::clone(&self.readiness)))
    }
}
