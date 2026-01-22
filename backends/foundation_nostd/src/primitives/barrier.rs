//! Spin-based barrier synchronization primitive.
//!
//! A barrier enables multiple threads to synchronize the beginning of some computation.

use core::fmt;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::primitives::spin_wait::SpinWait;

/// A barrier enables multiple threads to synchronize the beginning
/// of some computation using spin-waiting.
///
/// # Examples
///
/// ```ignore
/// use foundation_nostd::primitives::SpinBarrier;
/// use std::sync::Arc;
/// use std::thread;
///
/// let barrier = Arc::new(SpinBarrier::new(3));
///
/// let mut handles = vec![];
/// for i in 0..3 {
///     let barrier = Arc::clone(&barrier);
///     handles.push(thread::spawn(move || {
///         println!("Thread {} before barrier", i);
///         barrier.wait();
///         println!("Thread {} after barrier", i);
///     }));
/// }
///
/// for handle in handles {
///     handle.join().unwrap();
/// }
/// ```
pub struct SpinBarrier {
    num_threads: usize,
    count: AtomicUsize,
    generation: AtomicUsize,
}

/// Result returned from `SpinBarrier::wait()`.
pub struct BarrierWaitResult {
    is_leader: bool,
}

impl BarrierWaitResult {
    /// Returns `true` if this thread was the last to reach the barrier.
    #[inline]
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.is_leader
    }
}

impl SpinBarrier {
    /// Creates a new barrier that can block a given number of threads.
    ///
    /// # Panics
    ///
    /// Panics if `n` is 0.
    #[must_use]
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "barrier count must be > 0");
        Self {
            num_threads: n,
            count: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Blocks the current thread until all threads have reached this point.
    ///
    /// Barriers are re-usable after all threads have rendezvoused once.
    /// The thread that completes the barrier (the last one to call `wait()`)
    /// will receive a `BarrierWaitResult` with `is_leader()` returning `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinBarrier;
    ///
    /// let barrier = SpinBarrier::new(2);
    /// // In thread 1:
    /// let result = barrier.wait();
    /// if result.is_leader() {
    ///     println!("I was the last thread!");
    /// }
    /// ```
    pub fn wait(&self) -> BarrierWaitResult {
        let local_gen = self.generation.load(Ordering::Acquire);

        // Increment the count
        let count = self.count.fetch_add(1, Ordering::AcqRel) + 1;

        if count < self.num_threads {
            // Not the last thread - wait for generation to change
            let mut spin_wait = SpinWait::new();
            while self.generation.load(Ordering::Acquire) == local_gen {
                spin_wait.spin();
            }
            BarrierWaitResult { is_leader: false }
        } else {
            // Last thread - reset count and increment generation
            self.count.store(0, Ordering::Release);
            self.generation.fetch_add(1, Ordering::Release);
            BarrierWaitResult { is_leader: true }
        }
    }
}

impl fmt::Debug for SpinBarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpinBarrier")
            .field("num_threads", &self.num_threads)
            .field("count", &self.count.load(Ordering::Relaxed))
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    /// WHY: Validates barrier construction
    /// WHAT: Creating a barrier with count should work
    #[test]
    fn test_new() {
        let barrier = SpinBarrier::new(3);
        assert_eq!(barrier.num_threads, 3);
    }

    /// WHY: Validates barrier panics on zero count
    /// WHAT: Creating barrier with 0 threads should panic
    #[test]
    #[should_panic(expected = "barrier count must be > 0")]
    fn test_new_zero_panics() {
        let _ = SpinBarrier::new(0);
    }

    /// WHY: Validates single thread barrier
    /// WHAT: Barrier with n=1 should immediately return leader
    #[test]
    fn test_single_thread() {
        let barrier = SpinBarrier::new(1);
        let result = barrier.wait();
        assert!(result.is_leader());
    }

    /// WHY: Validates barrier is reusable
    /// WHAT: Barrier should work for multiple rounds
    #[test]
    fn test_reusable() {
        let barrier = SpinBarrier::new(1);

        // First round
        let result1 = barrier.wait();
        assert!(result1.is_leader());

        // Second round
        let result2 = barrier.wait();
        assert!(result2.is_leader());
    }

    /// WHY: Validates Debug implementation
    /// WHAT: Debug formatting should work
    #[test]
    fn test_debug() {
        let barrier = SpinBarrier::new(3);
        let debug = format!("{barrier:?}");
        assert!(debug.contains("SpinBarrier"));
        assert!(debug.contains("num_threads"));
    }
}
