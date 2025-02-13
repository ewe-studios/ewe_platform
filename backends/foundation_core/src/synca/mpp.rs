use std::{sync::Arc, time::Instant};

use concurrent_queue::{ConcurrentQueue, ForcePushError, PopError, PushError, TryIter};
use derive_more::derive::From;

pub struct Receiver<T> {
    chan: Arc<ConcurrentQueue<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq, From)]
pub enum ReceiverError {
    Timeout,
    Empty,
    Closed(PopError),
}

impl ReceiverError {
    pub fn is_timeout(&self) -> bool {
        matches!(self, ReceiverError::Timeout)
    }
}

impl core::error::Error for ReceiverError {}

impl core::fmt::Display for ReceiverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReceiverError::Closed(err) => write!(f, "ReceiverError::Closed({})", err),
            ReceiverError::Timeout => write!(f, "ReceiverError::Timeout"),
            ReceiverError::Empty => write!(f, "ReceiverError::Empty"),
        }
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            chan: self.chan.clone(),
        }
    }
}

#[allow(unused)]
impl<T> Receiver<T> {
    pub fn new(chan: Arc<ConcurrentQueue<T>>) -> Self {
        Receiver { chan }
    }

    pub fn capacity(&self) -> Option<usize> {
        self.chan.capacity()
    }

    pub fn len(&self) -> usize {
        self.chan.len()
    }

    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    pub fn close(&self) -> bool {
        self.chan.close()
    }

    pub fn is_full(&self) -> bool {
        self.chan.is_full()
    }

    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }

    pub fn try_iter(&self) -> TryIter<'_, T> {
        self.chan.try_iter()
    }

    pub fn recv(&self) -> Result<T, ReceiverError> {
        match self.chan.pop() {
            Ok(value) => Ok(value),
            Err(err) => match err {
                PopError::Empty => Err(ReceiverError::Empty),
                PopError::Closed => Err(ReceiverError::Closed(err)),
            },
        }
    }

    /// recv_timeout attempts to read value from the channel within the specified duration.
    /// It internally uses [thread::park_timeout] to block the current thread
    /// for a given duration until a value is received or the timeout is reached.
    fn recv_timeout(&self, dur: std::time::Duration) -> Result<T, ReceiverError> {
        let started = Instant::now();

        let mut remaining_timeout = dur.clone();
        loop {
            // if not parking then
            match self.chan.pop() {
                Ok(value) => return Ok(value),
                Err(err) => match err {
                    PopError::Empty => {
                        // check the state and see if we've crossed that threshold.
                        let elapsed = started.elapsed();
                        if elapsed >= remaining_timeout {
                            return Err(ReceiverError::Timeout);
                        }
                        remaining_timeout = remaining_timeout - elapsed;
                    }
                    PopError::Closed => return Err(ReceiverError::Closed(err)),
                },
            };

            std::thread::park_timeout(remaining_timeout);
        }
    }
}

pub struct Sender<T> {
    chan: Arc<ConcurrentQueue<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq, From)]
pub enum SenderError<T> {
    SendError(PushError<T>),
    ForceSendError(ForcePushError<T>),
}

impl<T: core::fmt::Debug> core::error::Error for SenderError<T> {}

impl<T: core::fmt::Debug> core::fmt::Display for SenderError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SenderError::SendError(err) => write!(f, "SenderError::SendError({:?})", err),
            SenderError::ForceSendError(err) => write!(f, "SenderError::ForceSendError({:?})", err),
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            chan: self.chan.clone(),
        }
    }
}

impl<T> Sender<T> {
    pub fn new(chan: Arc<ConcurrentQueue<T>>) -> Self {
        Sender { chan }
    }

    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }

    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    pub fn close(&self) -> bool {
        self.chan.close()
    }

    pub fn len(&self) -> usize {
        self.chan.len()
    }

    pub fn capacity(&self) -> Option<usize> {
        self.chan.capacity()
    }

    pub fn is_full(&self) -> bool {
        self.chan.is_full()
    }

    pub fn send(&self, value: T) -> Result<(), SenderError<T>> {
        match self.chan.push(value) {
            Ok(v) => Ok(v),
            Err(err) => Err(SenderError::SendError(err)),
        }
    }

    pub fn force_send(&self, value: T) -> Result<Option<T>, SenderError<T>> {
        match self.chan.force_push(value) {
            Ok(v) => Ok(v),
            Err(err) => Err(SenderError::ForceSendError(err)),
        }
    }
}

/// bounded creates a new bounded channel with the specified capacity.
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let chan = Arc::new(ConcurrentQueue::bounded(capacity));
    let sender = Sender::new(chan.clone());
    let receiver = Receiver::new(chan);
    (sender, receiver)
}

/// unbounded creates a new unbounded channel.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let chan = Arc::new(ConcurrentQueue::unbounded());
    let sender = Sender::new(chan.clone());
    let receiver = Receiver::new(chan);
    (sender, receiver)
}

#[cfg(test)]
mod test_channels {
    use std::{thread, time::Duration};

    use super::{bounded, unbounded, ReceiverError};

    #[test]
    fn unbounded_channel() {
        let (sender, receiver) = unbounded();
        sender.send(42).unwrap();
        assert_eq!(receiver.recv().unwrap(), 42);
    }

    #[test]
    fn bounded_channel() {
        let (sender, receiver) = bounded(1);
        sender.send(42).unwrap();
        assert_eq!(receiver.recv().unwrap(), 42);
    }

    #[test]
    fn fail_to_receive_item_after_timeout() {
        let (sender, receiver) = bounded(1);

        let sender_clone = sender.clone();
        let join_handler = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            sender_clone.send(42).unwrap();
        });

        assert!(
            matches!(
                receiver.recv_timeout(Duration::from_millis(50)),
                Err(ReceiverError::Timeout)
            ),
            "should fail to receive item after timeout"
        );

        join_handler.join().expect("should finish");
    }

    #[test]
    fn can_receive_item_after_timeout() {
        let (sender, receiver) = bounded(1);

        let sender_clone = sender.clone();
        let join_handler = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            sender_clone.send(42).unwrap();
        });

        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(102)).unwrap(),
            42
        );
        join_handler.join().expect("should finish");
    }
}
