// SignalTask — valtron TaskIterator for signal reception.

use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{Ctx, QueueReadiness, TaskIterator, TaskStatus};

use crate::signal::{SignalBus, SignalEvent, SignalHandle};

#[allow(dead_code)]
pub struct SignalTask {
    bus: Arc<SignalBus>,
    my_queue: Arc<ConcurrentQueue<SignalEvent>>,
    my_ready: QueueReadiness<SignalEvent>,
    handle: SignalHandle,
}

impl SignalTask {
    pub fn new(bus: Arc<SignalBus>, handle: SignalHandle) -> Self {
        let my_queue = bus.subscribe();
        let my_ready = QueueReadiness::new(my_queue.clone());
        Self {
            bus,
            my_queue,
            my_ready,
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
        // Park until signal arrives — uses the platform-specific handle
        Some(TaskStatus::Depends(Arc::new(self.handle.clone())))
    }
}
