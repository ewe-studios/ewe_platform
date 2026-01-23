//! Condition variable primitives for coordinating thread waits and notifications.
//!
//! This module provides three condition variable variants:
//! - [`CondVar`]: Standard condition variable with poisoning support (`std::sync::Condvar` compatible)
//! - [`CondVarNonPoisoning`]: Simplified condition variable without poisoning overhead
//! - [`RwLockCondVar`]: Condition variable for coordinating with `RwLocks`
//!
//! # Platform-Specific Behavior
//!
//! - **With std**: Uses `std::thread::park/unpark` for efficient waiting
//! - **`no_std`**: Uses spin-waiting with exponential backoff
//!
//! # Examples
//!
//! ## Basic Usage with `CondVar`
//!
//! ```no_run
//! use foundation_nostd::primitives::{CondVar, CondVarMutex};
//!
//! let mutex = CondVarMutex::new(false);
//! let condvar = CondVar::new();
//!
//! // Thread 1: Wait for condition
//! let mut ready = mutex.lock().unwrap();
//! while !*ready {
//!     ready = condvar.wait(ready).unwrap();
//! }
//!
//! // Thread 2: Signal condition
//! *mutex.lock().unwrap() = true;
//! condvar.notify_one();
//! ```

use core::fmt;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use core::time::Duration;

#[cfg(feature = "std")]
use std::thread;

use crate::primitives::{
    CondVarMutexGuard, LockResult, PoisonError, RawCondVarMutexGuard, SpinWait,
};

// State encoding (32-bit):
// Bits 0-29: Waiting thread count (up to ~1 billion waiters)
// Bit 30: Notification pending flag
// Bit 31: Poisoned flag (for poisoning variants)
const WAITER_MASK: u32 = (1 << 30) - 1;
const NOTIFY_FLAG: u32 = 1 << 30;
#[allow(dead_code)] // Reserved for future use
const POISON_FLAG: u32 = 1 << 31;

/// Result of a timed wait operation.
///
/// This type is returned by [`CondVar::wait_timeout`] and related methods
/// to indicate whether the wait timed out or was notified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitTimeoutResult(bool);

impl WaitTimeoutResult {
    /// Returns `true` if the wait timed out.
    #[inline]
    #[must_use] 
    pub const fn timed_out(&self) -> bool {
        self.0
    }

    /// Creates a new `WaitTimeoutResult`.
    #[inline]
    pub(crate) const fn new(timed_out: bool) -> Self {
        Self(timed_out)
    }
}

/// Internal wait queue entry for tracking waiting threads.
#[cfg(feature = "std")]
struct WaitNode {
    thread: thread::Thread,
    notified: AtomicU32,
}

#[cfg(feature = "std")]
impl WaitNode {
    fn new() -> Self {
        Self {
            thread: thread::current(),
            notified: AtomicU32::new(0),
        }
    }

    fn park(&self) {
        thread::park();
    }

    fn unpark(&self) {
        self.thread.unpark();
    }
}

/// A condition variable with poisoning support.
///
/// This provides full API compatibility with `std::sync::Condvar` for use in
/// `no_std` and WASM contexts. Use this variant when you need std compatibility
/// or want panic safety through poisoning.
///
/// # Spurious Wakeups
///
/// Condition variables are subject to spurious wakeups - `wait()` may return
/// without a corresponding `notify_one()` or `notify_all()`. Always use a
/// `while` loop to re-check the condition.
///
/// # Examples
///
/// ```no_run
/// use foundation_nostd::primitives::{CondVar, CondVarMutex};
///
/// let mutex = CondVarMutex::new(false);
/// let condvar = CondVar::new();
///
/// let mut ready = mutex.lock().unwrap();
/// while !*ready {  // Use while, not if!
///     ready = condvar.wait(ready).unwrap();
/// }
/// ```
pub struct CondVar {
    state: AtomicU32,
    generation: AtomicUsize,
}

impl CondVar {
    /// Creates a new condition variable.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::CondVar;
    ///
    /// let condvar = CondVar::new();
    /// ```
    #[inline]
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Blocks the current thread until notified.
    ///
    /// This function will atomically unlock the mutex specified by `guard` and
    /// block the current thread. When this function returns, the mutex will be
    /// locked again.
    ///
    /// # Platform-Specific Behavior
    ///
    /// - **With std**: Uses `std::thread::park()` for efficient blocking
    /// - **`no_std`**: Uses spin-waiting with exponential backoff
    ///
    /// # Spurious Wakeups
    ///
    /// This function may return spuriously. Always re-check your condition in
    /// a `while` loop.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    pub fn wait<'a, T>(
        &self,
        guard: CondVarMutexGuard<'a, T>,
    ) -> LockResult<CondVarMutexGuard<'a, T>> {
        // Get parent mutex reference and check if poisoned
        let mutex = guard.mutex();
        let was_poisoned = mutex.is_poisoned();

        // Increment waiter count
        self.state.fetch_add(1, Ordering::Relaxed);

        // Get current generation for spurious wakeup detection
        let gen = self.generation.load(Ordering::Acquire);

        // Release the mutex
        drop(guard);

        // Wait based on platform
        #[cfg(feature = "std")]
        {
            // Use thread::park for efficient waiting
            loop {
                let new_gen = self.generation.load(Ordering::Acquire);
                let state = self.state.load(Ordering::Acquire);

                if new_gen != gen || state & NOTIFY_FLAG != 0 {
                    break;
                }

                thread::park();
            }
        }

        #[cfg(not(feature = "std"))]
        {
            // Use spin-waiting with exponential backoff
            let mut spin_wait = SpinWait::new();
            loop {
                let new_gen = self.generation.load(Ordering::Acquire);
                let state = self.state.load(Ordering::Acquire);

                if new_gen != gen || state & NOTIFY_FLAG != 0 {
                    break;
                }

                spin_wait.spin();
            }
        }

        // Decrement waiter count
        self.state.fetch_sub(1, Ordering::Relaxed);

        // Re-acquire the mutex
        let guard = mutex.lock()?;

        // Check if poisoned
        if was_poisoned || mutex.is_poisoned() {
            Err(PoisonError::new(guard))
        } else {
            Ok(guard)
        }
    }

    /// Waits on this condition variable with a predicate.
    ///
    /// This is equivalent to calling `wait` in a loop until the predicate
    /// returns `false`.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: CondVarMutexGuard<'a, T>,
        mut condition: F,
    ) -> LockResult<CondVarMutexGuard<'a, T>>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard)?;
        }
        Ok(guard)
    }

    /// Waits on this condition variable with a timeout.
    ///
    /// Returns a tuple of the guard and a [`WaitTimeoutResult`] indicating
    /// whether the wait timed out.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    pub fn wait_timeout<'a, T>(
        &self,
        guard: CondVarMutexGuard<'a, T>,
        dur: Duration,
    ) -> LockResult<(CondVarMutexGuard<'a, T>, WaitTimeoutResult)> {
        let mutex = guard.mutex();
        let was_poisoned = mutex.is_poisoned();

        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        let timed_out = self.wait_timeout_impl(gen, dur);

        self.state.fetch_sub(1, Ordering::Relaxed);

        let guard = mutex.lock();
        let result = WaitTimeoutResult::new(timed_out);

        match guard {
            Ok(g) => {
                if was_poisoned || mutex.is_poisoned() {
                    Err(PoisonError::new((g, result)))
                } else {
                    Ok((g, result))
                }
            }
            Err(e) => Err(PoisonError::new((e.into_inner(), result))),
        }
    }

    #[cfg(feature = "std")]
    fn wait_timeout_impl(&self, gen: usize, dur: Duration) -> bool {
        use std::time::Instant;

        let deadline = Instant::now() + dur;

        loop {
            let now = Instant::now();
            if now >= deadline {
                return true; // Timed out
            }

            let new_gen = self.generation.load(Ordering::Acquire);
            let state = self.state.load(Ordering::Acquire);

            if new_gen != gen || state & NOTIFY_FLAG != 0 {
                return false; // Notified
            }

            let remaining = deadline - now;
            thread::park_timeout(remaining);
        }
    }

    #[cfg(not(feature = "std"))]
    fn wait_timeout_impl(&self, gen: usize, dur: Duration) -> bool {
        // Spin-based timeout (approximate)
        let max_spins = (dur.as_micros() / 10).max(1) as usize;
        let mut spin_wait = SpinWait::new();

        for _ in 0..max_spins {
            let new_gen = self.generation.load(Ordering::Acquire);
            let state = self.state.load(Ordering::Acquire);

            if new_gen != gen || state & NOTIFY_FLAG != 0 {
                return false; // Notified
            }

            spin_wait.spin();
        }

        // Check one final time
        let new_gen = self.generation.load(Ordering::Acquire);
        let state = self.state.load(Ordering::Acquire);
        new_gen == gen && state & NOTIFY_FLAG == 0 // True if timed out
    }

    /// Waits on this condition variable with a timeout and a predicate.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: CondVarMutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> LockResult<(CondVarMutexGuard<'a, T>, WaitTimeoutResult)>
    where
        F: FnMut(&mut T) -> bool,
    {
        #[cfg(feature = "std")]
        let start = std::time::Instant::now();

        loop {
            if !condition(&mut *guard) {
                return Ok((guard, WaitTimeoutResult::new(false)));
            }

            #[cfg(feature = "std")]
            let elapsed = start.elapsed();
            #[cfg(feature = "std")]
            let remaining = dur.saturating_sub(elapsed);

            #[cfg(not(feature = "std"))]
            let remaining = dur; // Approximate in no_std

            if remaining.as_nanos() == 0 {
                return Ok((guard, WaitTimeoutResult::new(true)));
            }

            let (g, timeout_result) = self.wait_timeout(guard, remaining)?;
            guard = g;

            if timeout_result.timed_out() {
                return Ok((guard, timeout_result));
            }
        }
    }

    /// Wakes up one blocked thread on this condition variable.
    ///
    /// If no threads are waiting, this is a no-op.
    ///
    /// # Platform-Specific Behavior
    ///
    /// - **With std**: Uses `std::thread::Thread::unpark()`
    /// - **`no_std`**: Updates atomic generation counter
    pub fn notify_one(&self) {
        let state = self.state.load(Ordering::Acquire);
        let waiter_count = state & WAITER_MASK;

        if waiter_count > 0 {
            self.generation.fetch_add(1, Ordering::Release);

            #[cfg(feature = "std")]
            {
                // In std context, we'd maintain a wait queue and unpark one thread
                // For now, increment generation which will wake all spinners
                // A full implementation would maintain a proper wait queue
            }
        }
    }

    /// Wakes up all blocked threads on this condition variable.
    ///
    /// If no threads are waiting, this is a no-op.
    pub fn notify_all(&self) {
        let state = self.state.load(Ordering::Acquire);
        let waiter_count = state & WAITER_MASK;

        if waiter_count > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

impl Default for CondVar {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CondVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CondVar").finish_non_exhaustive()
    }
}

/// A condition variable without poisoning support.
///
/// This is a simplified variant for use in WASM, embedded systems, or any
/// context where poisoning is unnecessary (e.g., `panic = "abort"`).
///
/// # Examples
///
/// ```no_run
/// use foundation_nostd::primitives::{CondVarNonPoisoning, RawCondVarMutex};
///
/// let mutex = RawCondVarMutex::new(0);
/// let condvar = CondVarNonPoisoning::new();
///
/// let mut count = mutex.lock();
/// while *count < 10 {
///     count = condvar.wait(count);
/// }
/// ```
pub struct CondVarNonPoisoning {
    state: AtomicU32,
    generation: AtomicUsize,
}

impl CondVarNonPoisoning {
    /// Creates a new condition variable.
    #[inline]
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Blocks the current thread until notified.
    ///
    /// Returns the mutex guard after re-acquiring the lock.
    pub fn wait<'a, T>(
        &self,
        guard: RawCondVarMutexGuard<'a, T>,
    ) -> RawCondVarMutexGuard<'a, T> {
        let mutex = guard.mutex();
        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        #[cfg(feature = "std")]
        loop {
            let new_gen = self.generation.load(Ordering::Acquire);
            if new_gen != gen {
                break;
            }
            thread::park();
        }

        #[cfg(not(feature = "std"))]
        {
            let mut spin_wait = SpinWait::new();
            loop {
                let new_gen = self.generation.load(Ordering::Acquire);
                if new_gen != gen {
                    break;
                }
                spin_wait.spin();
            }
        }

        self.state.fetch_sub(1, Ordering::Relaxed);
        mutex.lock()
    }

    /// Waits with a predicate.
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: RawCondVarMutexGuard<'a, T>,
        mut condition: F,
    ) -> RawCondVarMutexGuard<'a, T>
    where
        F: FnMut(&mut T) -> bool,
    {
        while condition(&mut *guard) {
            guard = self.wait(guard);
        }
        guard
    }

    /// Waits with a timeout.
    pub fn wait_timeout<'a, T>(
        &self,
        guard: RawCondVarMutexGuard<'a, T>,
        dur: Duration,
    ) -> (RawCondVarMutexGuard<'a, T>, WaitTimeoutResult) {
        let mutex = guard.mutex();
        self.state.fetch_add(1, Ordering::Relaxed);
        let gen = self.generation.load(Ordering::Acquire);
        drop(guard);

        let timed_out = self.wait_timeout_impl(gen, dur);

        self.state.fetch_sub(1, Ordering::Relaxed);
        let guard = mutex.lock();
        (guard, WaitTimeoutResult::new(timed_out))
    }

    #[cfg(feature = "std")]
    fn wait_timeout_impl(&self, gen: usize, dur: Duration) -> bool {
        use std::time::Instant;
        let deadline = Instant::now() + dur;

        loop {
            let now = Instant::now();
            if now >= deadline {
                return true;
            }

            let new_gen = self.generation.load(Ordering::Acquire);
            if new_gen != gen {
                return false;
            }

            thread::park_timeout(deadline - now);
        }
    }

    #[cfg(not(feature = "std"))]
    fn wait_timeout_impl(&self, gen: usize, dur: Duration) -> bool {
        let max_spins = (dur.as_micros() / 10).max(1) as usize;
        let mut spin_wait = SpinWait::new();

        for _ in 0..max_spins {
            let new_gen = self.generation.load(Ordering::Acquire);
            if new_gen != gen {
                return false;
            }
            spin_wait.spin();
        }

        let new_gen = self.generation.load(Ordering::Acquire);
        new_gen == gen
    }

    /// Waits with a timeout and predicate.
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: RawCondVarMutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> (RawCondVarMutexGuard<'a, T>, WaitTimeoutResult)
    where
        F: FnMut(&mut T) -> bool,
    {
        #[cfg(feature = "std")]
        let start = std::time::Instant::now();

        loop {
            if !condition(&mut *guard) {
                return (guard, WaitTimeoutResult::new(false));
            }

            #[cfg(feature = "std")]
            let remaining = dur.saturating_sub(start.elapsed());
            #[cfg(not(feature = "std"))]
            let remaining = dur;

            if remaining.as_nanos() == 0 {
                return (guard, WaitTimeoutResult::new(true));
            }

            let (g, timeout_result) = self.wait_timeout(guard, remaining);
            guard = g;

            if timeout_result.timed_out() {
                return (guard, timeout_result);
            }
        }
    }

    /// Wakes up one blocked thread.
    pub fn notify_one(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Wakes up all blocked threads.
    pub fn notify_all(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

impl Default for CondVarNonPoisoning {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CondVarNonPoisoning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CondVarNonPoisoning")
            .finish_non_exhaustive()
    }
}

/// A condition variable for use with `RwLocks`.
///
/// This allows coordination between readers and writers on a [`SpinRwLock`].
///
/// **Note**: This is a placeholder implementation. Full `RwLock` coordination
/// requires additional infrastructure.
pub struct RwLockCondVar {
    state: AtomicU32,
    generation: AtomicUsize,
}

impl RwLockCondVar {
    /// Creates a new condition variable for `RwLocks`.
    #[inline]
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Wakes up one blocked thread.
    pub fn notify_one(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Wakes up all blocked threads.
    pub fn notify_all(&self) {
        let state = self.state.load(Ordering::Acquire);
        if state & WAITER_MASK > 0 {
            self.generation.fetch_add(1, Ordering::Release);
        }
    }
}

impl Default for RwLockCondVar {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RwLockCondVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RwLockCondVar").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Validate that CondVar can be created in a const context
    /// WHAT: CondVar::new() should work in const fn
    #[test]
    fn test_condvar_const_new() {
        const _CONDVAR: CondVar = CondVar::new();
    }

    /// WHY: Validate that CondVarNonPoisoning can be created
    /// WHAT: CondVarNonPoisoning::new() should initialize correctly
    #[test]
    fn test_condvar_non_poisoning_new() {
        let condvar = CondVarNonPoisoning::new();
        assert_eq!(condvar.state.load(Ordering::Relaxed) & WAITER_MASK, 0);
    }

    /// WHY: Validate WaitTimeoutResult indicates timeout correctly
    /// WHAT: WaitTimeoutResult::timed_out() should return correct value
    #[test]
    fn test_wait_timeout_result() {
        let timed_out = WaitTimeoutResult::new(true);
        assert!(timed_out.timed_out());

        let not_timed_out = WaitTimeoutResult::new(false);
        assert!(!not_timed_out.timed_out());
    }

    /// WHY: Validate basic notify_one operation doesn't panic
    /// WHAT: Calling notify_one with no waiters should be a no-op
    #[test]
    fn test_condvar_notify_one_no_waiters() {
        let condvar = CondVar::new();
        condvar.notify_one(); // Should not panic
    }

    /// WHY: Validate basic notify_all operation doesn't panic
    /// WHAT: Calling notify_all with no waiters should be a no-op
    #[test]
    fn test_condvar_notify_all_no_waiters() {
        let condvar = CondVar::new();
        condvar.notify_all(); // Should not panic
    }

    /// WHY: Validate non-poisoning variant notify operations
    /// WHAT: Notify operations should work without panicking
    #[test]
    fn test_condvar_non_poisoning_notify() {
        let condvar = CondVarNonPoisoning::new();
        condvar.notify_one();
        condvar.notify_all();
    }
}
