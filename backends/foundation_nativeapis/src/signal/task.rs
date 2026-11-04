// SignalTask — valtron TaskIterator for signal reception.

use std::sync::Arc;

use foundation_core::valtron::{Ctx, EventQueue, TaskIterator, TaskStatus};

use crate::signal::{SignalBus, SignalEvent, SignalHandle};

#[allow(dead_code)]
pub struct SignalTask {
    bus: Arc<SignalBus>,
    my_queue: EventQueue<SignalEvent>,
    handle: SignalHandle,
}

impl SignalTask {
    pub fn new(bus: Arc<SignalBus>, handle: SignalHandle) -> Self {
        let my_queue = bus.subscribe_events();
        Self {
            bus,
            my_queue,
            handle,
        }
    }
}

impl TaskIterator for SignalTask {
    type Ready = SignalEvent;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Drain received signals
        if let Ok(event) = self.my_queue.pop() {
            return Some(TaskStatus::Ready(event));
        }
        // Park until a signal arrives: the delivery pushes to the queue, which
        // wakes this task. (The platform handle's eventfd is never drained, so a
        // park on it would be ready again at once after the first signal.)
        let _ = &self.handle;
        Some(TaskStatus::Depends(Arc::new(self.my_queue.readiness())))
    }
}
