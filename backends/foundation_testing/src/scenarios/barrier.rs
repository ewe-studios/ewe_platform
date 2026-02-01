//! Barrier synchronization using `CondVar`.

use foundation_nostd::primitives::{CondVar, CondVarMutex};
use std::sync::Arc;

/// A barrier that blocks threads until all threads have reached it.
///
/// # Examples
///
/// ```
/// use foundation_testing::scenarios::Barrier;
/// use std::thread;
/// use std::sync::Arc;
///
/// let barrier = Arc::new(Barrier::new(3));
/// let mut handles = vec![];
///
/// for i in 0..3 {
///     let barrier_clone = Arc::clone(&barrier);
///     handles.push(thread::spawn(move || {
///         println!("Thread {} before barrier", i);
///         barrier_clone.wait();
///         println!("Thread {} after barrier", i);
///     }));
/// }
///
/// for handle in handles {
///     handle.join().unwrap();
/// }
/// ```
pub struct Barrier {
    inner: Arc<Inner>,
}

struct Inner {
    state: CondVarMutex<BarrierState>,
    condvar: CondVar,
    num_threads: usize,
}

struct BarrierState {
    count: usize,
    generation: usize,
}

impl Barrier {
    /// Creates a new barrier for the given number of threads.
    ///
    /// # Panics
    ///
    /// Panics if `num_threads` is 0.
    #[must_use]
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "Barrier num_threads must be > 0");

        Self {
            inner: Arc::new(Inner {
                state: CondVarMutex::new(BarrierState {
                    count: 0,
                    generation: 0,
                }),
                condvar: CondVar::new(),
                num_threads,
            }),
        }
    }

    /// Blocks the current thread until all threads have called `wait()`.
    ///
    /// Returns `true` for one thread (the "leader") and `false` for all others.
    ///
    /// # Panics
    ///
    /// Panics if the mutex or condition variable is poisoned.
    #[must_use] 
    pub fn wait(&self) -> bool {
        let mut guard = match self.inner.state.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        let local_gen = guard.generation;
        guard.count += 1;

        if guard.count >= self.inner.num_threads {
            // Last thread to arrive - wake everyone and reset
            guard.count = 0;
            guard.generation += 1;
            drop(guard);
            self.inner.condvar.notify_all();
            true // Leader
        } else {
            // Wait for all threads to arrive
            while guard.generation == local_gen {
                guard = match self.inner.condvar.wait(guard) {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
            }
            false // Follower
        }
    }
}

impl Clone for Barrier {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
