// SignalBus — fan-out to multiple subscribers via ConcurrentQueue.

use std::sync::{Arc, Mutex};

use concurrent_queue::ConcurrentQueue;

use crate::signal::event::SignalEvent;

pub struct SignalBus {
    subscribers: std::sync::Mutex<Vec<Arc<ConcurrentQueue<SignalEvent>>>>,
}

impl Clone for SignalBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl SignalBus {
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// Subscribe — returns a queue that receives all signal events.
    pub fn subscribe(&self) -> Arc<ConcurrentQueue<SignalEvent>> {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        self.subscribers.lock().unwrap().push(queue.clone());
        queue
    }

    /// Deliver a signal event to all subscribers.
    pub(crate) fn deliver(&self, event: SignalEvent) {
        let subs = self.subscribers.lock().unwrap();
        for sub in subs.iter() {
            let _ = sub.push(event.clone());
        }
    }
}

impl Default for SignalBus {
    fn default() -> Self {
        Self::new()
    }
}
