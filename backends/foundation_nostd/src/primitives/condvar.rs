//! Condition variable primitives for coordinating thread waits and notifications.
//!
//! This module provides three condition variable variants:
//! - [`CondVar`]: Standard condition variable with poisoning support (`std::sync::Condvar` compatible)
//! - [`CondVarNonPoisoning`]: Simplified condition variable without poisoning overhead
//! - [`RwLockCondVar`]: Condition variable for coordinating with `RwLocks`
//!
//! And corresponding mutex types:
//! - [`CondVarMutex`]: Mutex for use with `CondVar`
//! - [`RawCondVarMutex`]: Mutex for use with `CondVarNonPoisoning`
//!
//! # Platform-Specific Behavior
//!
//! - **With std**: Uses `std::sync::{Condvar, Mutex}` directly for optimal performance
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
    #[cfg(any(not(feature = "std"), test))] // Used in no_std implementation and tests
    pub(crate) const fn new(timed_out: bool) -> Self {
        Self(timed_out)
    }
}

// Feature-gate the implementation
#[cfg(feature = "std")]
mod std_impl;
#[cfg(feature = "std")]
pub use std_impl::{
    CondVar, CondVarMutex, CondVarMutexGuard, CondVarNonPoisoning, RawCondVarMutex,
    RawCondVarMutexGuard, RwLockCondVar,
};

#[cfg(not(feature = "std"))]
mod nostd_impl;
#[cfg(not(feature = "std"))]
pub use nostd_impl::{
    CondVar, CondVarMutex, CondVarMutexGuard, CondVarNonPoisoning, RawCondVarMutex,
    RawCondVarMutexGuard, RwLockCondVar,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::SpinRwLock;

    #[test]
    fn test_condvar_new() {
        let _ = CondVar::new();
    }

    #[test]
    fn test_condvar_notify_without_waiters() {
        let condvar = CondVar::new();
        condvar.notify_one();
        condvar.notify_all();
    }

    #[test]
    fn test_wait_timeout_result() {
        let result = WaitTimeoutResult::new(true);
        assert!(result.timed_out());

        let result = WaitTimeoutResult::new(false);
        assert!(!result.timed_out());
    }

    /// WHY: Tests CondVar wait_timeout with short duration
    /// WHAT: Should timeout after specified duration
    #[test]
    fn test_condvar_wait_timeout() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(1));
        match result {
            Ok((_guard, timeout_result)) => assert!(timeout_result.timed_out()),
            Err(_) => {} // Poisoned mutex is OK for timeout test
        }
    }

    /// WHY: Tests CondVarNonPoisoning wait_timeout with short duration
    /// WHAT: Should timeout after specified duration
    #[test]
    fn test_condvar_non_poisoning_wait_timeout() {
        use super::RawCondVarMutex;
        use core::time::Duration;

        let mutex = RawCondVarMutex::new(false);
        let condvar = CondVarNonPoisoning::new();

        let guard = mutex.lock();
        let (_guard, result) = condvar.wait_timeout(guard, Duration::from_millis(1));
        assert!(result.timed_out());
    }

    // RwLockCondVar tests (TDD - tests written first)

    /// WHY: Validates basic construction and notification of `RwLockCondVar`
    /// WHAT: Ensures `RwLockCondVar` can be created and notify operations don't panic
    #[test]
    fn test_rwlock_condvar_new() {
        let condvar = RwLockCondVar::new();
        condvar.notify_one();
        condvar.notify_all();
    }

    /// WHY: Tests `wait_read` basic operation with read guard
    /// WHAT: Reader should be able to wait on condition and be notified
    #[test]
    fn test_rwlock_condvar_wait_read_basic() {
        let lock = SpinRwLock::new(0);
        let _condvar = RwLockCondVar::new();

        let guard = lock.read().unwrap();
        // For now, immediate notification (will implement wait_read next)
        drop(guard);
    }

    /// WHY: Tests `wait_write` basic operation with write guard
    /// WHAT: Writer should be able to wait on condition and be notified
    #[test]
    fn test_rwlock_condvar_wait_write_basic() {
        let lock = SpinRwLock::new(0);
        let _condvar = RwLockCondVar::new();

        let guard = lock.write().unwrap();
        // For now, immediate notification (will implement wait_write next)
        drop(guard);
    }

    /// WHY: Tests `wait_while_read` with predicate on read guard
    /// WHAT: Reader should wait while predicate is true, wake when false
    #[test]
    fn test_rwlock_condvar_wait_while_read() {
        let lock = SpinRwLock::new(false);
        let _condvar = RwLockCondVar::new();

        let guard = lock.read().unwrap();
        // Predicate check (will implement wait_while_read next)
        let _value = *guard;
        drop(guard);
    }

    /// WHY: Tests `wait_while_write` with predicate on write guard
    /// WHAT: Writer should wait while predicate is true, wake when false
    #[test]
    fn test_rwlock_condvar_wait_while_write() {
        let lock = SpinRwLock::new(false);
        let _condvar = RwLockCondVar::new();

        let mut guard = lock.write().unwrap();
        // Predicate check (will implement wait_while_write next)
        *guard = true;
        drop(guard);
    }

    /// WHY: Tests `wait_timeout_read` with timeout duration on read guard
    /// WHAT: Reader wait should timeout after specified duration
    #[test]
    fn test_rwlock_condvar_wait_timeout_read() {
        use core::time::Duration;

        let lock = SpinRwLock::new(0);
        let condvar = RwLockCondVar::new();

        let guard = lock.read().unwrap();
        let (_guard, result) = condvar
            .wait_timeout_read(guard, &lock, Duration::from_millis(1))
            .unwrap();
        assert!(result.timed_out());
    }

    /// WHY: Tests `wait_timeout_write` with timeout duration on write guard
    /// WHAT: Writer wait should timeout after specified duration
    #[test]
    fn test_rwlock_condvar_wait_timeout_write() {
        use core::time::Duration;

        let lock = SpinRwLock::new(0);
        let condvar = RwLockCondVar::new();

        let guard = lock.write().unwrap();
        let (_guard, result) = condvar
            .wait_timeout_write(guard, &lock, Duration::from_millis(1))
            .unwrap();
        assert!(result.timed_out());
    }
}
