use std::{
    sync::Arc,
    time::{self, Instant},
};

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
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, ReceiverError::Timeout)
    }
}

impl core::error::Error for ReceiverError {}

impl core::fmt::Display for ReceiverError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReceiverError::Closed(err) => write!(f, "ReceiverError::Closed({err})"),
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

    #[must_use]
    pub fn capacity(&self) -> Option<usize> {
        self.chan.capacity()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    #[must_use]
    pub fn close(&self) -> bool {
        self.chan.close()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.chan.is_full()
    }

    #[must_use]
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

    /// `recv_timeout` attempts to read value from the channel within the specified duration.
    /// It internally uses [`thread::park_timeout`] to block the current thread
    /// for a given duration until a value is received or the timeout is reached.
    pub fn recv_timeout(&self, dur: std::time::Duration) -> Result<T, ReceiverError> {
        let started = Instant::now();

        let mut remaining_timeout = dur;
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
                        remaining_timeout -= elapsed;
                    }
                    PopError::Closed => return Err(ReceiverError::Closed(err)),
                },
            }

            std::thread::park_timeout(remaining_timeout);
        }
    }
}

pub struct RecvIter<T> {
    chan: Arc<ConcurrentQueue<T>>,
}

impl<T> Clone for RecvIter<T> {
    fn clone(&self) -> Self {
        Self {
            chan: self.chan.clone(),
        }
    }
}

#[allow(unused)]
impl<T> RecvIter<T> {
    pub fn new(chan: Arc<ConcurrentQueue<T>>) -> Self {
        Self { chan }
    }

    /// [`as_iter`] returns a new iterator that will block for 50 nanoseconds
    /// if the channel is empty and will yield to the OS thread scheduler if no
    /// content is received.
    #[must_use]
    pub fn as_iter(self) -> RecvIterator<T> {
        RecvIterator::ten_nano(self)
    }

    /// [`block_iter`] returns a new iterator that will block for provided duration
    /// if the channel is empty and will yield to the OS thread scheduler if no
    /// content is received.
    #[must_use]
    pub fn block_iter(self, dur: time::Duration) -> RecvIterator<T> {
        RecvIterator::new(self, dur)
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    /// [`recv`] returns a Result of the value from the underlying channel if
    /// there is a value or if empty or closed.
    pub fn recv(&self) -> Result<T, ReceiverError> {
        match self.chan.pop() {
            Ok(value) => Ok(value),
            Err(err) => match err {
                PopError::Empty => Err(ReceiverError::Empty),
                PopError::Closed => Err(ReceiverError::Closed(err)),
            },
        }
    }

    /// [`block_recv`] blocks the thread with a spinning loop waiting for an item
    /// to be received on the channel.
    pub fn block_recv(&self, block_ts: time::Duration) -> Result<T, ReceiverError> {
        let mut yield_now = false;
        loop {
            // if not parking then
            match self.chan.pop() {
                Ok(value) => return Ok(value),
                Err(err) => match err {
                    PopError::Empty => {
                        if yield_now {
                            yield_now = false;
                            std::thread::yield_now();
                            continue;
                        }

                        std::thread::park_timeout(block_ts);
                        yield_now = true;
                    }
                    PopError::Closed => {
                        return Err(ReceiverError::Closed(err));
                    }
                },
            };
        }
    }
}

/// [`RecvIterator`] implements an iterator for the [`RecvIter`] type.
///
/// The [`time::Duration`] received indicates how long it will block to get
/// the next value upon which it will yield, if no value is received before
/// that period the `RecvIterator` returns `None` if there was any error or if the internal
/// receiver's channel was closed.
///
/// Which provides a sensible polling functionality yielding the threading when the timeout hits
/// and no data is received.
pub struct RecvIterator<T>(RecvIter<T>, time::Duration);

/// [`DEFAULT_BLOCK_DURATION`] is the default wait time used by the [`RecvIter`]
/// when we use a normal iterator.
const DEFAULT_BLOCK_DURATION: time::Duration = time::Duration::from_nanos(50);

impl<T> RecvIterator<T> {
    pub fn from_chan(item: Arc<ConcurrentQueue<T>>, dur: time::Duration) -> Self {
        Self::new(RecvIter::new(item), dur)
    }

    pub fn from_ten_nano(item: Arc<ConcurrentQueue<T>>) -> Self {
        Self::new(RecvIter::new(item), DEFAULT_BLOCK_DURATION)
    }

    #[must_use]
    pub fn ten_nano(item: RecvIter<T>) -> Self {
        Self::new(item, DEFAULT_BLOCK_DURATION)
    }

    #[must_use]
    pub fn new(item: RecvIter<T>, dur: time::Duration) -> Self {
        Self(item, dur)
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> Iterator for RecvIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.block_recv(self.1).ok()
    }
}

pub enum Stream<D, P> {
    // Indicative the stream is instantiating.
    Init,

    // Indicative of internal system operations occuring that should be ignored.
    Ignore,

    // Indicative that the stream next response will be delayed by this much duration
    Delayed(std::time::Duration),

    // Indicating that the stream is a pending state with giving context value.
    Pending(P),

    // Indicative the stream just issued its next value.
    Next(D),
}

/// [`StreamIterator`] defines a type which implements an
/// iterator that returns a stream stream
/// of values.
pub trait StreamIterator<D, P>: Iterator<Item = Stream<D, P>> {}

pub struct StreamRecvIterator<D, P>(RecvIterator<Stream<D, P>>);

impl<D, P> StreamRecvIterator<D, P> {
    #[must_use]
    pub fn new(iter: RecvIterator<Stream<D, P>>) -> Self {
        Self(iter)
    }
}

impl<D, P> Iterator for StreamRecvIterator<D, P> {
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<D, P> StreamIterator<D, P> for StreamRecvIterator<D, P> {}

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
            SenderError::SendError(err) => write!(f, "SenderError::SendError({err:?})"),
            SenderError::ForceSendError(err) => write!(f, "SenderError::ForceSendError({err:?})"),
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

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    #[must_use]
    pub fn close(&self) -> bool {
        self.chan.close()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    #[must_use]
    pub fn capacity(&self) -> Option<usize> {
        self.chan.capacity()
    }

    #[must_use]
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
#[must_use]
pub fn bounded<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let chan = Arc::new(ConcurrentQueue::bounded(capacity));
    let sender = Sender::new(chan.clone());
    let receiver = Receiver::new(chan);
    (sender, receiver)
}

/// unbounded creates a new unbounded channel.
#[must_use]
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let chan = Arc::new(ConcurrentQueue::unbounded());
    let sender = Sender::new(chan.clone());
    let receiver = Receiver::new(chan);
    (sender, receiver)
}

#[cfg(test)]
mod test_channels {
    use std::{sync::Arc, thread, time::Duration};

    use concurrent_queue::ConcurrentQueue;

    use crate::synca::mpp::RecvIterator;

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

    #[test]
    fn can_receive_from_recv_iterator() {
        let chan: Arc<ConcurrentQueue<usize>> = Arc::new(ConcurrentQueue::bounded(10));

        chan.push(42).expect("Must receive value");
        chan.close();

        let chan_iter = RecvIterator::from_ten_nano(chan);

        let items: Vec<usize> = chan_iter.collect();
        dbg!("Received values: {:?}", &items);

        assert_eq!(items, vec![42]);
    }
}
