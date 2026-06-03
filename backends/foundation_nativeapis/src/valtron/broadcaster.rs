/// Multi-subscriber broadcaster built on top of mpp channels.
///
/// Each subscriber gets an independent `Receiver<T>`.
/// When `broadcast()` is called, the event is pushed to every subscriber's queue.
/// Dead subscriber channels (Receiver dropped) are cleaned up automatically.

use foundation_core::synca::mpp::{self, Receiver, Sender};

pub struct EventBroadcaster<T: Clone + Send + 'static> {
    subscribers: Vec<Sender<T>>,
    capacity: usize,
}

impl<T: Clone + Send + 'static> EventBroadcaster<T> {
    /// Create a new broadcaster with the given per-subscriber channel capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            subscribers: Vec::new(),
            capacity,
        }
    }

    /// Subscribe — returns a (Sender<T>, Receiver<T>) pair.
    ///
    /// The Sender is also returned so the subscriber can explicitly close
    /// their own channel if needed.
    pub fn subscribe(&mut self) -> (Sender<T>, Receiver<T>) {
        let (tx, rx) = mpp::bounded(self.capacity);
        self.subscribers.push(tx.clone());
        (tx, rx)
    }

    /// Send an event to all subscribers.
    ///
    /// Dead subscriber channels (Receiver dropped) are cleaned up automatically.
    /// Uses `force_send` to drop oldest events if a subscriber's queue is full.
    pub fn broadcast(&mut self, event: T) {
        self.subscribers.retain(|tx| {
            // If the channel is closed (receiver dropped), try_send will fail
            match tx.send(event.clone()) {
                Ok(()) => true,
                Err(_) => {
                    // Channel is full or closed — try force_send
                    match tx.force_send(event.clone()) {
                        Ok(_dropped) => true, // Queue full, dropped oldest but still alive
                        Err(_) => false,      // Channel closed — remove this subscriber
                    }
                }
            }
        });
    }

    /// Clean up dead subscribers (Receiver dropped).
    pub fn cleanup(&mut self) {
        self.subscribers.retain(|tx| !tx.is_closed());
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

impl<T: Clone + Send + 'static> Default for EventBroadcaster<T> {
    fn default() -> Self {
        Self::new(64)
    }
}
