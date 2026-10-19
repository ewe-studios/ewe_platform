//! WaitGroup - Thread completion tracking primitive.
//!
//! WHY: Provides a counting barrier for tracking N outstanding work items.
//!
//! WHAT: `WaitGroup` blocks until count reaches 0. `WaitGroupGuard` is a
//! RAII guard that calls `done()` on drop, ensuring cleanup even on panic.
//!
//! HOW: A `Mutex<usize>` holds the count and a `Condvar` parks waiters. The
//! count is read and the thread parks ATOMICALLY under the same lock, so a
//! `done()` that drives the count to 0 between a waiter's check and its park
//! cannot be lost (no lost-wakeup). An earlier implementation used an atomic
//! count plus a separate `LockSignal`, which had exactly that race: the
//! signal could fire in the gap between the count check and the park, and
//! `lock_and_wait()`'s internal `try_lock()` then clobbered the signalled
//! state — leaving the waiter parked forever. Surfaced as intermittent
//! "test running for over 60 seconds" hangs under contention.

use std::sync::{Arc, Condvar, Mutex};

/// Tracks N outstanding work items. `wait()` blocks until count reaches 0.
///
/// Each worker thread gets a `WaitGroupGuard` that calls `done()` on drop,
/// ensuring cleanup even if the thread panics.
///
/// # Examples
///
/// ```
/// use foundation_core::synca::WaitGroup;
/// use std::sync::Arc;
///
/// let wg = WaitGroup::new();
/// wg.add(3); // 3 workers
///
/// for i in 0..3 {
///     let wg = wg.clone();
///     std::thread::spawn(move || {
///         let _guard = wg.guard(); // done() called on drop
///         // do work
///     });
/// }
///
/// wg.wait(); // block until all workers done
/// ```
#[derive(Clone)]
pub struct WaitGroup {
    /// (count, condvar). The count lives INSIDE the mutex so that reading it
    /// and parking on the condvar happen atomically under the same lock.
    inner: Arc<(Mutex<usize>, Condvar)>,
}

impl WaitGroup {
    /// Create a new `WaitGroup` with count 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }

    /// Increment the counter by n.
    pub fn add(&self, n: usize) {
        let (lock, _) = &*self.inner;
        let mut count = lock.lock().unwrap_or_else(|p| p.into_inner());
        *count += n;
    }

    /// Decrement the counter. If it reaches 0, wake all waiters.
    pub fn done(&self) {
        let (lock, cond) = &*self.inner;
        let mut count = lock.lock().unwrap_or_else(|p| p.into_inner());
        debug_assert!(*count > 0, "WaitGroup::done() called more times than add()");
        *count -= 1;
        if *count == 0 {
            // Notify under the lock — a waiter checking the predicate cannot be
            // between its check and its park, because both happen under this lock.
            cond.notify_all();
        }
    }

    /// Block until count reaches 0.
    pub fn wait(&self) {
        let (lock, cond) = &*self.inner;
        let mut count = lock.lock().unwrap_or_else(|p| p.into_inner());
        while *count != 0 {
            count = cond.wait(count).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// Block until count reaches 0 or `timeout` elapses.
    ///
    /// Returns `true` if the count reached 0 within the deadline, `false` if it
    /// timed out with work still outstanding. Used for bounded drains (e.g. a
    /// graceful-shutdown grace window) where the caller force-closes whatever
    /// remains past the deadline. The deadline is tracked absolutely so
    /// spurious condvar wakeups shorten the remaining wait rather than resetting
    /// it.
    #[must_use]
    pub fn wait_timeout(&self, timeout: std::time::Duration) -> bool {
        let (lock, cond) = &*self.inner;
        let deadline = std::time::Instant::now() + timeout;
        let mut count = lock.lock().unwrap_or_else(|p| p.into_inner());
        while *count != 0 {
            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                return false;
            };
            let (next, res) = cond
                .wait_timeout(count, remaining)
                .unwrap_or_else(|p| p.into_inner());
            count = next;
            if res.timed_out() && *count != 0 {
                return false;
            }
        }
        true
    }

    /// Create a RAII guard that calls `done()` on drop.
    #[must_use]
    pub fn guard(&self) -> WaitGroupGuard {
        WaitGroupGuard(self.clone())
    }
}

impl Default for WaitGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Calls `WaitGroup::done()` on drop — ensures threads that panic still decrement.
pub struct WaitGroupGuard(WaitGroup);

impl Drop for WaitGroupGuard {
    fn drop(&mut self) {
        self.0.done();
    }
}
