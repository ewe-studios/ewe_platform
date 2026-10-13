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
/// ```
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
    pub num_threads: usize,
    /// Packed barrier state: the low `count_bits` bits hold the number of
    /// threads that have arrived in the current generation, the remaining
    /// high bits hold the (wrapping) generation number.
    ///
    /// Both live in one atomic so that a thread's arrival and the generation
    /// it arrived in are observed together. With separate atomics, a thread
    /// arriving between the leader resetting the count and bumping the
    /// generation would record the *old* generation and be released by that
    /// bump without its own generation ever filling up.
    state: AtomicUsize,
    count_bits: u32,
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
    /// Panics if `n` is 0, or if `n` is so large (more than `usize::MAX / 2`)
    /// that no bits would remain to track the barrier generation.
    #[must_use]
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "barrier count must be > 0");
        // Bits needed to store any arrival count in `0..n`.
        let count_bits = usize::BITS - n.leading_zeros();
        assert!(
            count_bits < usize::BITS,
            "barrier count must leave room for a generation counter"
        );
        Self {
            num_threads: n,
            state: AtomicUsize::new(0),
            count_bits,
        }
    }

    #[inline]
    fn count_of(&self, state: usize) -> usize {
        state & ((1usize << self.count_bits) - 1)
    }

    #[inline]
    fn generation_of(&self, state: usize) -> usize {
        state >> self.count_bits
    }

    /// Blocks the current thread until all threads have reached this point.
    ///
    /// Barriers are re-usable after all threads have rendezvoused once.
    /// The thread that completes the barrier (the last one to call `wait()`)
    /// will receive a `BarrierWaitResult` with `is_leader()` returning `true`.
    ///
    /// `wait()` only returns once `n` threads (the count given to
    /// [`SpinBarrier::new`]) have called it, so a barrier for `n > 1` must be
    /// waited on from `n` different threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinBarrier;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let barrier = Arc::new(SpinBarrier::new(2));
    ///
    /// let other = {
    ///     let barrier = Arc::clone(&barrier);
    ///     thread::spawn(move || barrier.wait().is_leader())
    /// };
    /// let this_is_leader = barrier.wait().is_leader();
    /// let other_is_leader = other.join().unwrap();
    ///
    /// // Exactly one of the two threads is the leader.
    /// assert!(this_is_leader ^ other_is_leader);
    /// ```
    pub fn wait(&self) -> BarrierWaitResult {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            let count = self.count_of(current);
            let generation = self.generation_of(current);

            if count + 1 >= self.num_threads {
                // Last thread: start the next generation with a zero count.
                // Shifting out the high bits wraps the generation counter.
                let next = generation.wrapping_add(1) << self.count_bits;
                match self.state.compare_exchange_weak(
                    current,
                    next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => return BarrierWaitResult { is_leader: true },
                    Err(actual) => current = actual,
                }
            } else {
                match self.state.compare_exchange_weak(
                    current,
                    current + 1,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Not the last thread: wait for the generation we
                        // arrived in to complete.
                        let mut spin_wait = SpinWait::new();
                        while self.generation_of(self.state.load(Ordering::Acquire)) == generation {
                            if !spin_wait.spin() {
                                // Backoff exhausted: keep hinting the CPU
                                // rather than hot-looping on the load.
                                spin_wait.spin_once();
                            }
                        }
                        return BarrierWaitResult { is_leader: false };
                    }
                    Err(actual) => current = actual,
                }
            }
        }
    }
}

impl fmt::Debug for SpinBarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.load(Ordering::Relaxed);
        f.debug_struct("SpinBarrier")
            .field("num_threads", &self.num_threads)
            .field("count", &self.count_of(state))
            .field("generation", &self.generation_of(state))
            .finish_non_exhaustive()
    }
}
