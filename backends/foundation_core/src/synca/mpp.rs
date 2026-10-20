use std::{
    sync::Arc,
    time::{self, Duration, Instant},
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

    /// Get a clone of the underlying channel Arc
    #[must_use]
    pub fn chan(&self) -> Arc<ConcurrentQueue<T>> {
        self.chan.clone()
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

    pub fn into_recv_iter_timeout(&self, duration: Duration) -> RecvIterator<T> {
        RecvIterator::from_chan(self.chan.clone(), duration)
    }

    pub fn into_recv_iter(&self) -> RecvIterator<T> {
        RecvIterator::from_hundred_millis(self.chan.clone())
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
    /// It internally uses [`std::thread::park_timeout`] to block the current thread
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

    /// [`Self::as_iter`] returns a new iterator that will block for 50 nanoseconds
    /// if the channel is empty and will yield to the OS thread scheduler if no
    /// content is received.
    #[must_use]
    pub fn as_iter(self) -> RecvIterator<T> {
        RecvIterator::ten_nano(self)
    }

    /// [`Self::block_iter`] returns a new iterator that will block for provided duration
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

    /// [`Self::recv`] returns a Result of the value from the underlying channel if
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

    /// [`Self::block_recv`] blocks the thread with a spinning loop waiting for an item
    /// to be received on the channel.
    pub fn block_recv(&self, block_ts: time::Duration) -> Result<T, ReceiverError> {
        let mut yield_now = false;
        loop {
            // if not parking then
            let val = self.chan.pop();
            match val {
                Ok(value) => return Ok(value),
                Err(err) => match err {
                    PopError::Empty => {
                        if yield_now {
                            yield_now = false;
                            std::thread::yield_now();
                            // std::hint::spin_loop();
                            continue;
                        }

                        // noticed this was way slower than std::hint::spin_loop
                        // or std::thread::sleep.
                        std::thread::park_timeout(block_ts);
                        yield_now = true;
                    }
                    PopError::Closed => {
                        return Err(ReceiverError::Closed(err));
                    }
                },
            }
        }
    }
}

/// `RecvIterator` implements an iterator for the [`RecvIter`] type.
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

    pub fn from_hundred_millis(item: Arc<ConcurrentQueue<T>>) -> Self {
        Self::new(RecvIter::new(item), Duration::from_millis(100))
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

pub struct Sender<T> {
    chan: Arc<ConcurrentQueue<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderError<T> {
    /// Queue is full - retry with backpressure
    Full(T),
    /// Queue is closed (receiver dropped) - stop sending
    Closed(T),
}

impl<T: core::fmt::Debug> core::error::Error for SenderError<T> {}

impl<T: core::fmt::Debug> core::fmt::Display for SenderError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SenderError::Full(_) => write!(f, "SenderError::Full"),
            SenderError::Closed(_) => write!(f, "SenderError::Closed"),
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
            Ok(()) => Ok(()),
            Err(PushError::Full(v)) => Err(SenderError::Full(v)),
            Err(PushError::Closed(v)) => Err(SenderError::Closed(v)),
        }
    }

    pub fn force_send(&self, value: T) -> Result<Option<T>, SenderError<T>> {
        match self.chan.force_push(value) {
            Ok(v) => Ok(v),
            Err(ForcePushError(v)) => Err(SenderError::Closed(v)),
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

/// Multi-subscriber broadcaster built on top of mpp channels.
///
/// Each subscriber gets an independent `Receiver<T>`.
/// When `broadcast()` is called, the event is pushed to every subscriber's queue.
/// Dead subscriber channels (Receiver dropped) are cleaned up automatically.
pub struct Broadcaster<T: Clone + Send + 'static> {
    subscribers: Vec<Sender<T>>,
    capacity: usize,
}

impl<T: Clone + Send + 'static> Broadcaster<T> {
    /// Create a new broadcaster with the given per-subscriber channel capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            subscribers: Vec::new(),
            capacity,
        }
    }

    /// Subscribe — returns a `Receiver<T>`.
    pub fn subscribe(&mut self) -> Receiver<T> {
        let (tx, rx) = bounded(self.capacity);
        self.subscribers.push(tx);
        rx
    }

    /// Send an event to all subscribers.
    ///
    /// Dead subscriber channels (Receiver dropped) are cleaned up automatically.
    /// Uses `force_send` to drop oldest events if a subscriber's queue is full.
    pub fn broadcast(&mut self, event: T) {
        self.subscribers.retain(|tx| {
            match tx.send(event.clone()) {
                Ok(()) => true,
                Err(_) => {
                    match tx.force_send(event.clone()) {
                        Ok(_dropped) => true,
                        Err(_) => false,
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

impl<T: Clone + Send + 'static> Default for Broadcaster<T> {
    fn default() -> Self {
        Self::new(64)
    }
}

// ---------------------------------------------------------------------------
// TrackedBroadcaster — &self-safe fan-out with delivery tracking & eviction
// ---------------------------------------------------------------------------

struct TrackedSlot<T> {
    sender: Sender<T>,
    delivered_up_to: u64,
    consecutive_failures: u32,
}

struct TrackedInner<T: Clone + Send + 'static> {
    slots: Vec<TrackedSlot<T>>,
    capacity: usize,
    max_retries: u32,
    event_index: u64,
}

/// `&self`-safe multi-subscriber broadcaster with bounded per-subscriber
/// queues, delivery tracking, and eviction on max-retry failure.
///
/// Unlike [`Broadcaster`] (which requires `&mut self`), all methods here take
/// `&self` — an internal `Mutex` handles synchronization. Each subscriber gets
/// an independent bounded `Receiver<T>`. `broadcast()` fans out via try-push
/// (never blocks). Subscribers that fail to keep up are evicted after
/// `max_retries` consecutive push failures; their queue is closed so the
/// consumer sees `PopError::Closed`.
pub struct TrackedBroadcaster<T: Clone + Send + 'static> {
    inner: std::sync::Mutex<TrackedInner<T>>,
}

impl<T: Clone + Send + 'static> TrackedBroadcaster<T> {
    pub fn new(capacity: usize) -> Self {
        Self::with_max_retries(capacity, 16)
    }

    pub fn with_max_retries(capacity: usize, max_retries: u32) -> Self {
        Self {
            inner: std::sync::Mutex::new(TrackedInner {
                slots: Vec::new(),
                capacity,
                max_retries,
                event_index: 0,
            }),
        }
    }

    pub fn subscribe(&self) -> Receiver<T> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let cap = inner.capacity;
        let idx = inner.event_index;
        let (tx, rx) = bounded(cap);
        inner.slots.push(TrackedSlot {
            sender: tx,
            delivered_up_to: idx,
            consecutive_failures: 0,
        });
        rx
    }

    /// Fan out an event to all subscribers. Never blocks.
    ///
    /// Try-push to each subscriber's queue. On success, reset the failure
    /// counter. On failure (queue full or closed), increment
    /// `consecutive_failures`. After `max_retries` consecutive failures,
    /// evict: close the queue and remove the slot.
    pub fn broadcast(&self, event: T) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.event_index += 1;
        let idx = inner.event_index;
        let max = inner.max_retries;

        inner.slots.retain_mut(|slot| {
            if slot.sender.is_closed() {
                return false;
            }
            match slot.sender.send(event.clone()) {
                Ok(()) => {
                    slot.delivered_up_to = idx;
                    slot.consecutive_failures = 0;
                    true
                }
                Err(_) => {
                    slot.consecutive_failures += 1;
                    if slot.consecutive_failures >= max {
                        let _ = slot.sender.close();
                        false
                    } else {
                        true
                    }
                }
            }
        });
    }

    pub fn subscriber_count(&self) -> usize {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.slots.len()
    }

    /// Close all subscriber queues (session-end lifecycle signal).
    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        for slot in &inner.slots {
            let _ = slot.sender.close();
        }
        inner.slots.clear();
    }
}

impl<T: Clone + Send + 'static> Default for TrackedBroadcaster<T> {
    fn default() -> Self {
        Self::new(64)
    }
}
