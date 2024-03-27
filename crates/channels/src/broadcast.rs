use crate::mspc;
use std::sync;

pub fn create<E: Send + 'static>(initial_subscribers_capacity: usize) -> Broadcast<E> {
    Broadcast::<E>::new(initial_subscribers_capacity)
}

// Broadcast is multi-produre multi-subscriber multi-cast implements
// that is an eager deliver-er of messages.
//
// It does not try to deliver the same amount of messages to all subscribers
// subscribed at varying times, if you were subscribed after some messages
// were sent then do not expect to get those messages.
pub struct Broadcast<E: Send + 'static> {
    message_receiver: mspc::ReceiveChannel<E>,
    message_sender: mspc::SendChannel<E>,
    subscribers: sync::Mutex<Vec<mspc::SendChannel<sync::Arc<E>>>>,
}

impl<E: Send + 'static> Broadcast<E> {
    pub(crate) fn new(initial_subscribers_capacity: usize) -> Self {
        let (message_sender, message_receiver) = mspc::create::<E>();

        return Self {
            message_sender,
            message_receiver,
            subscribers: sync::Mutex::new(Vec::with_capacity(initial_subscribers_capacity)),
        };
    }

    pub fn has_pending_messages(&mut self) -> bool {
        !self.message_receiver.is_empty().unwrap()
    }

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
        subscribers.push(sender)
    }

    fn deliver_pending_messages(&mut self) {
        let mut result = self.subscribers.try_lock();
        if result.is_err() {
            return;
        }

        let mut subs = result.unwrap();
        if subs.len() == 0 {
            return;
        }

        while let Ok(message) = self.message_receiver.try_receive() {
            let message_reference = sync::Arc::new(message);
            for sub in subs.iter_mut() {
                sub.try_send(message_reference.clone())
                    .expect("should be able to send sender pending message")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::broadcast;

    #[test]
    fn broadcast_should_cache_pending_messages_when_no_subscribers() {
        let mut broadcaster = broadcast::create::<String>(5);

        broadcaster.broadcast(String::from("first"));
        broadcaster.broadcast(String::from("second"));

        assert!(broadcaster.has_pending_messages())
    }

    #[test]
    fn broadcast_should_cache_pending_messages_until_atleast_one_subscriber_joins() {
        let mut broadcaster = broadcast::create::<String>(5);

        broadcaster.broadcast(String::from("first"));
        broadcaster.broadcast(String::from("second"));

        assert!(broadcaster.has_pending_messages());

        let mut subscriber = broadcaster.subscribe();
        assert!(!subscriber.is_empty().unwrap());

        assert!(!broadcaster.has_pending_messages());
    }

    #[test]
    fn broadcast_should_immediately_all_pending_messages_to_subscribers() {
        let mut broadcaster = broadcast::create::<String>(5);

        let mut subscriber = broadcaster.subscribe();
        assert!(subscriber.is_empty().unwrap());

        broadcaster.broadcast(String::from("first"));
        broadcaster.broadcast(String::from("second"));

        assert!(!broadcaster.has_pending_messages());
        assert!(!subscriber.is_empty().unwrap());
    }

    #[test]
    fn broadcast_new_subscribers_should_miss_already_sent_messages() {
        let mut broadcaster = broadcast::create::<String>(5);

        let mut subscriber = broadcaster.subscribe();

        broadcaster.broadcast(String::from("first"));
        broadcaster.broadcast(String::from("second"));

        assert!(!broadcaster.has_pending_messages());
        assert!(!subscriber.is_empty().unwrap());

        let mut subscriber2 = broadcaster.subscribe();
        assert!(subscriber2.is_empty().unwrap());
    }

    #[test]
    fn broadcast_new_subscribers_should_miss_already_sent_messages_but_get_new_messages() {
        let mut broadcaster = broadcast::create::<String>(5);

        let mut subscriber = broadcaster.subscribe();

        broadcaster.broadcast(String::from("first"));

        assert!(!broadcaster.has_pending_messages());
        assert!(!subscriber.is_empty().unwrap());

        let mut subscriber2 = broadcaster.subscribe();

        broadcaster.broadcast(String::from("second"));

        assert!(!subscriber2.is_empty().unwrap());
    }
}
