//! Producer-consumer queue implementation using CondVar.

use foundation_nostd::primitives::{CondVar, CondVarMutex};
use std::sync::Arc;
use std::time::Duration;

/// A thread-safe producer-consumer queue using condition variables.
///
/// # Examples
///
/// ```
/// use foundation_testing::scenarios::ProducerConsumerQueue;
/// use std::thread;
///
/// let queue = ProducerConsumerQueue::new(10);
///
/// // Producer thread
/// let queue_clone = queue.clone();
/// let producer = thread::spawn(move || {
///     for i in 0..5 {
///         queue_clone.push(i);
///     }
/// });
///
/// // Consumer thread
/// let queue_clone = queue.clone();
/// let consumer = thread::spawn(move || {
///     for _ in 0..5 {
///         let _item = queue_clone.pop();
///     }
/// });
///
/// producer.join().unwrap();
/// consumer.join().unwrap();
/// ```
#[derive(Clone)]
pub struct ProducerConsumerQueue<T> {
    inner: Arc<Inner<T>>,
}

const DEFAULT_MAX_POP_WAIT: usize = 100;
const DEFAULT_WAIT_DURATION: Duration = Duration::from_millis(100);

struct Inner<T> {
    queue: CondVarMutex<Vec<T>>,
    not_empty: CondVar,
    not_full: CondVar,
    capacity: usize,
    max_pop_wait: usize,
    wait_duration: Duration,
}

impl<T> ProducerConsumerQueue<T> {
    /// Creates a new producer-consumer queue with the given capacity.
    #[must_use]
    pub fn with_max_loop(capacity: usize, max_pop_wait: usize, wait_duration: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                queue: CondVarMutex::new(Vec::with_capacity(capacity)),
                not_empty: CondVar::new(),
                not_full: CondVar::new(),
                max_pop_wait,
                wait_duration,
                capacity,
            }),
        }
    }

    /// Creates a new producer-consumer queue with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self::with_max_loop(capacity, DEFAULT_MAX_POP_WAIT, DEFAULT_WAIT_DURATION)
    }

    /// Pushes an item to the queue, blocking if the queue is full.
    ///
    /// It will block until it can push the item into the queue.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn push(&self, item: T) {
        let mut guard = match self.inner.queue.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        // Wait while queue is full

        while guard.len() >= self.inner.capacity {
            guard = match self.inner.not_full.wait(guard) {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
        }

        guard.push(item);

        // Notify consumers that queue is not empty
        drop(guard);
        self.inner.not_empty.notify_all();
    }

    /// Pops an item from the queue, blocking if the queue is empty.
    ///
    /// Will loop until the maximum allowed loop runs and returns None
    /// if no item is inserted within that period, this ensures we never loop
    /// forever, blocking the thread, you can wrap this in a while loop to
    /// get the forever looping semantics but you need to be careful to ensure
    /// you can adequate exit such a state.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn pop(&self) -> Option<T> {
        let mut guard = match self.inner.queue.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        // Wait while queue is empty
        let mut looped: usize = self.inner.max_pop_wait;
        while guard.is_empty() {
            if looped == 0 {
                return None;
            }

            looped -= 1;
            guard = match self
                .inner
                .not_empty
                .wait_timeout(guard, self.inner.wait_duration)
            {
                Ok((g, _)) => g,
                Err(e) => {
                    let (g, _) = e.into_inner();
                    g
                }
            };
        }

        let item = guard.pop().expect("Queue was empty after wait");

        // Notify producers that queue is not full
        drop(guard);
        self.inner.not_full.notify_all();

        Some(item)
    }

    /// [`wake_one`] sends a wake signal to one threads.
    pub fn wake_one(&self) {
        self.inner.not_full.notify_one();
    }

    /// [`wake_all`] sends a wake signal to all threads.
    pub fn wake_all(&self) {
        self.inner.not_full.notify_all();
    }

    /// Returns the current number of items in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        match self.inner.queue.lock() {
            Ok(g) => g.len(),
            Err(e) => e.into_inner().len(),
        }
    }

    /// Returns true if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the capacity of the queue.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }
}
