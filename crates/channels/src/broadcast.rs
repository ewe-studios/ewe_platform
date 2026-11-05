// #![allow(clippy::)]

use crate::mspc::{self, ChannelError};
use std::sync;

#[must_use]
pub fn create<E: Send + 'static>(initial_subscribers_capacity: usize) -> Broadcast<E> {
    Broadcast::<E>::new(initial_subscribers_capacity)
}

pub type Subscribers<E> = sync::Arc<sync::Mutex<Vec<Option<mspc::SendChannel<sync::Arc<E>>>>>>;

/// Broadcast is multi-produre multi-subscriber multi-cast implements
/// that is an eager deliver-er of messages.
///
/// It does not try to deliver the same amount of messages to all subscribers
/// subscribed at varying times, if you were subscribed after some messages
/// were sent then do not expect to get those messages.
///
// TODO(alex.ewetumo): One thing we do need to test in the wild is how the vec
// grows for this implementation has for now we do not clean up Option<SendChannel>
// with where replaced the content with None for closed senders.
pub struct Broadcast<E: Send + 'static> {
    message_receiver: mspc::ReceiveChannel<E>,
    message_sender: mspc::SendChannel<E>,
    subscribers: Subscribers<E>,
}

impl<E: Send + 'static> Clone for Broadcast<E> {
    fn clone(&self) -> Self {
        Self {
            message_receiver: self.message_receiver.clone(),
            message_sender: self.message_sender.clone(),
            subscribers: self.subscribers.clone(),
        }
    }
}

impl<E: Send + 'static> Broadcast<E> {
    #[must_use]
    pub fn new(initial_subscribers_capacity: usize) -> Self {
        let (message_sender, message_receiver) = mspc::create::<E>();

        Self {
            message_sender,
            message_receiver,
            subscribers: sync::Arc::new(sync::Mutex::new(Vec::with_capacity(
                initial_subscribers_capacity,
            ))),
        }
    }

    /// # Panics
    ///
    /// Panics if the internal receiver is in an invalid state.
    pub fn has_pending_messages(&mut self) -> bool {
        !self.message_receiver.is_empty().unwrap()
    }

    /// # Panics
    ///
    /// Panics if the message cannot be sent to the internal queue.
    pub fn broadcast(&mut self, item: E) {
        self.message_sender
            .try_send(item)
            .expect("should have delivered message to queue");
        self.deliver_pending_messages();
    }

    pub fn subscribe(&mut self) -> mspc::ReceiveChannel<sync::Arc<E>> {
        let (sender, receiver) = mspc::create::<sync::Arc<E>>();
        self.add_and_deliver_pending_messages(sender);
        receiver
    }

    fn add_and_deliver_pending_messages(&mut self, sender: mspc::SendChannel<sync::Arc<E>>) {
        self.add_subscriber_sender(sender);
        self.deliver_pending_messages();
    }

    fn add_subscriber_sender(&mut self, sender: mspc::SendChannel<sync::Arc<E>>) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.push(Some(sender));
    }

    fn deliver_pending_messages(&mut self) {
        let mut subs = self.subscribers.try_lock().unwrap();
        if subs.is_empty() {
            return;
        }

        while let Ok(message) = self.message_receiver.try_receive() {
            let message_reference = sync::Arc::new(message);
            for sub_slot in subs.iter_mut() {
                if let Some(sub) = sub_slot {
                    // if its closed, then remove sender.
                    if let Err(trace) = sub.try_send(message_reference.clone()) {
                        if matches!(trace.current_context(), ChannelError::Closed) {
                            sub_slot.take();
                        }
                    }
                }
            }
        }
    }
}
