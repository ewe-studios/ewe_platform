//! Producer-consumer queue implementation using CondVar.

use foundation_nostd::primitives::{CondVar, CondVarMutex};
use std::sync::Arc;

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

struct Inner<T> {
    queue: CondVarMutex<Vec<T>>,
    not_empty: CondVar,
    not_full: CondVar,
    capacity: usize,
}

impl<T> ProducerConsumerQueue<T> {
    /// Creates a new producer-consumer queue with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                queue: CondVarMutex::new(Vec::with_capacity(capacity)),
                not_empty: CondVar::new(),
                not_full: CondVar::new(),
                capacity,
            }),
        }
    }

    /// Pushes an item to the queue, blocking if the queue is full.
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
        self.inner.not_empty.notify_one();
    }

    /// Pops an item from the queue, blocking if the queue is empty.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn pop(&self) -> T {
        let mut guard = match self.inner.queue.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        // Wait while queue is empty
        while guard.is_empty() {
            guard = match self.inner.not_empty.wait(guard) {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
        }

        let item = guard.pop().expect("Queue was empty after wait");

        // Notify producers that queue is not full
        drop(guard);
        self.inner.not_full.notify_one();

        item
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
