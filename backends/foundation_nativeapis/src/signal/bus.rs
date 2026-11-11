// SignalBus — fan-out to multiple subscribers via ConcurrentQueue.

use std::sync::{Arc, Mutex};

use concurrent_queue::ConcurrentQueue;
use foundation_context::Ctx;
use foundation_core::valtron::EventQueue;

use crate::signal::event::SignalEvent;

pub struct SignalBus {
    subscribers: std::sync::Mutex<Vec<Arc<ConcurrentQueue<SignalEvent>>>>,
    /// Subscribers that are valtron tasks: a delivered event wakes the task
    /// parked on the queue's readiness.
    event_subscribers: std::sync::Mutex<Vec<EventQueue<SignalEvent>>>,
    ctx: Ctx,
}

impl Clone for SignalBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: std::sync::Mutex::new(Vec::new()),
            event_subscribers: std::sync::Mutex::new(Vec::new()),
            ctx: self.ctx.clone(),
        }
    }
}

impl SignalBus {
    /// A bus whose event timestamps read the clock of `ctx`.
    #[must_use]
    pub fn new(ctx: Ctx) -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            event_subscribers: Mutex::new(Vec::new()),
            ctx,
        }
    }

    /// The context that stamps every delivered event.
    pub(crate) fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    /// Subscribe — returns a queue that receives all signal events.
    ///
    /// # Panics
    /// Panics if the subscriber list lock is poisoned.
    pub fn subscribe(&self) -> Arc<ConcurrentQueue<SignalEvent>> {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        self.subscribers.lock().unwrap().push(queue.clone());
        queue
    }

    /// Subscribe a valtron task — returns a queue that receives all signal events
    /// and wakes the task parked on its readiness (`EventQueue::readiness`).
    ///
    /// # Panics
    /// Panics if the subscriber list lock is poisoned.
    pub fn subscribe_events(&self) -> EventQueue<SignalEvent> {
        let queue = EventQueue::unbounded();
        self.event_subscribers.lock().unwrap().push(queue.clone());
        queue
    }

    /// Deliver a signal event to all subscribers.
    pub(crate) fn deliver(&self, event: &SignalEvent) {
        let subs = self.subscribers.lock().unwrap();
        for sub in subs.iter() {
            let _ = sub.push(event.clone());
        }
        for sub in self.event_subscribers.lock().unwrap().iter() {
            let _ = sub.push(event.clone());
        }
    }
}
