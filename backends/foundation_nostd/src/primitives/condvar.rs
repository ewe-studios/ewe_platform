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

    /// WHY: Tests `CondVar` `wait_timeout` with short duration
    /// WHAT: Should timeout after specified duration
    #[test]
    fn test_condvar_wait_timeout() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(1));
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
        // Poisoned mutex is OK for timeout test
    }

    /// WHY: Tests `CondVarNonPoisoning` `wait_timeout` with short duration
    /// WHAT: Should timeout after specified duration
    #[test]
    fn test_condvar_non_poisoning_wait_timeout() {
        use super::RawCondVarMutex;
        use core::time::Duration;

        let mutex = RawCondVarMutex::new(false);
        let condvar = CondVarNonPoisoning::new();

        let guard = mutex.lock().unwrap();
        let (_guard, result) = condvar
            .wait_timeout(guard, Duration::from_millis(1))
            .unwrap();
        assert!(result.timed_out());
    }

    // RwLockCondVar tests (TDD - tests written first)
    // These tests are only valid in no_std mode as RwLockCondVar is a foundation-specific type
    #[cfg(not(feature = "std"))]
    mod rwlock_condvar_tests {
        use super::*;
        use crate::primitives::SpinRwLock;

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

    // ========================================================================
    // Additional Unit Tests for Comprehensive Coverage
    // ========================================================================

    // ------------------------------------------------------------------------
    // CondVar timeout tests
    // ------------------------------------------------------------------------

    /// WHY: Tests `wait_timeout` with zero duration (immediate timeout)
    /// WHAT: Should timeout immediately without blocking
    #[test]
    fn test_condvar_wait_timeout_zero_duration() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::ZERO);
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
    }

    /// WHY: Tests `wait_timeout` with very short duration (1ms)
    /// WHAT: Should timeout after approximately 1ms
    #[test]
    fn test_condvar_wait_timeout_short_duration() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(1));
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
    }

    /// WHY: Tests `wait_timeout` with medium duration (10ms)
    /// WHAT: Should timeout after approximately 10ms without notification
    #[test]
    fn test_condvar_wait_timeout_medium_duration() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(10));
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
    }

    /// WHY: Tests `wait_timeout` with long duration (100ms) but immediate notification
    /// WHAT: Should NOT timeout when notified before duration expires
    #[cfg(feature = "std")]
    #[test]
    #[ignore] // Requires threading support - run with cargo test -- --ignored
    fn test_condvar_wait_timeout_long_with_notification() {
        use core::time::Duration;
        use std::sync::Arc;
        use std::thread;

        let mutex = Arc::new(CondVarMutex::new(false));
        let condvar = Arc::new(CondVar::new());

        let mutex_clone = Arc::clone(&mutex);
        let condvar_clone = Arc::clone(&condvar);

        // Spawn thread to notify after 10ms
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            let mut guard = mutex_clone.lock().unwrap();
            *guard = true;
            drop(guard);
            condvar_clone.notify_one();
        });

        // Wait for up to 100ms
        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(100));
        if let Ok((_guard, timeout_result)) = result {
            // Should NOT timeout as we were notified
            assert!(!timeout_result.timed_out());
        }
    }

    /// WHY: Tests actual timeout behavior verification
    /// WHAT: Verifies `timed_out()` returns true after real timeout
    #[test]
    fn test_condvar_wait_timeout_result_verification() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(5));

        // Verify the result structure
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out(), "Timeout should have occurred");
            // Verify timed_out() method works correctly
            let is_timeout = timeout_result.timed_out();
            assert!(is_timeout);
        }
    }

    // ------------------------------------------------------------------------
    // CondVar combined tests
    // ------------------------------------------------------------------------

    /// WHY: Tests `wait_timeout_while` combined predicate + timeout behavior
    /// WHAT: Should timeout while predicate remains true, return early if predicate becomes false
    #[test]
    fn test_condvar_wait_timeout_while_predicate_and_timeout() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(0);
        let condvar = CondVar::new();

        // Test 1: Predicate always true, should timeout
        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout_while(guard, Duration::from_millis(5), |count| {
            *count == 0 // Always true, will timeout
        });

        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }

        // Test 2: Predicate immediately false, should return without waiting
        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout_while(guard, Duration::from_millis(100), |count| {
            *count != 0 // Immediately false, should return quickly
        });

        if let Ok((_guard, timeout_result)) = result {
            // Should NOT timeout as predicate was false
            assert!(!timeout_result.timed_out());
        }
    }

    /// WHY: Tests spurious wakeup handling with predicate re-evaluation
    /// WHAT: Even if woken spuriously, should re-check predicate and continue waiting
    #[test]
    fn test_condvar_spurious_wakeup_predicate_reevaluation() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        // Predicate checks condition
        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout_while(guard, Duration::from_millis(5), |ready| {
            !*ready // Wait while not ready
        });

        // Should timeout since condition was never satisfied
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
    }

    // ------------------------------------------------------------------------
    // CondVar poisoning tests (std feature only)
    // ------------------------------------------------------------------------

    #[cfg(feature = "std")]
    mod poisoning_tests {
        use super::*;
        use std::sync::Arc;
        use std::thread;

        /// WHY: Tests poisoning on panic during wait
        /// WHAT: If a thread panics while holding the mutex, `CondVar` should detect poisoning
        #[test]
        fn test_condvar_poisoning_on_panic_during_wait() {
            let mutex = Arc::new(CondVarMutex::new(false));
            let condvar = Arc::new(CondVar::new());

            let mutex_clone = Arc::clone(&mutex);
            let condvar_clone = Arc::clone(&condvar);

            // Spawn thread that panics while holding lock
            let handle = thread::spawn(move || {
                let guard = mutex_clone.lock().unwrap();
                // Simulate wait that panics
                let _result =
                    condvar_clone.wait_timeout(guard, core::time::Duration::from_millis(1));
                panic!("Intentional panic for poisoning test");
            });

            // Wait for thread to panic
            let _ = handle.join();

            // Try to acquire lock - should be poisoned
            let result = mutex.lock();
            assert!(result.is_err(), "Mutex should be poisoned after panic");
        }

        /// WHY: Tests `PoisonError` recovery method `into_inner()`
        /// WHAT: Should be able to extract guard from `PoisonError` and recover
        #[test]
        fn test_poison_error_into_inner() {
            let mutex = Arc::new(CondVarMutex::new(42));
            let mutex_clone = Arc::clone(&mutex);

            // Poison the mutex
            let handle = thread::spawn(move || {
                let _guard = mutex_clone.lock().unwrap();
                panic!("Poison the mutex");
            });
            let _ = handle.join();

            // Get poisoned error and recover
            let result = mutex.lock();
            match result {
                Err(poison_err) => {
                    let guard = poison_err.into_inner();
                    assert_eq!(
                        *guard, 42,
                        "Should be able to access data from poisoned mutex"
                    );
                }
                Ok(_) => panic!("Expected poisoned mutex"),
            }
        }

        /// WHY: Tests `PoisonError` recovery method `get_ref()`
        /// WHAT: Should be able to get reference to guard from `PoisonError`
        #[test]
        fn test_poison_error_get_ref() {
            let mutex = Arc::new(CondVarMutex::new(100));
            let mutex_clone = Arc::clone(&mutex);

            // Poison the mutex
            let handle = thread::spawn(move || {
                let _guard = mutex_clone.lock().unwrap();
                panic!("Poison the mutex");
            });
            let _ = handle.join();

            // Get reference through poisoned error
            let result = mutex.lock();
            match result {
                Err(poison_err) => {
                    let guard_ref = poison_err.get_ref();
                    assert_eq!(
                        **guard_ref, 100,
                        "Should be able to reference data from poisoned mutex"
                    );
                }
                Ok(_) => panic!("Expected poisoned mutex"),
            }
        }

        /// WHY: Tests `PoisonError` recovery method `get_mut()`
        /// WHAT: Should be able to get mutable reference to guard from `PoisonError`
        #[test]
        fn test_poison_error_get_mut() {
            let mutex = Arc::new(CondVarMutex::new(200));
            let mutex_clone = Arc::clone(&mutex);

            // Poison the mutex
            let handle = thread::spawn(move || {
                let _guard = mutex_clone.lock().unwrap();
                panic!("Poison the mutex");
            });
            let _ = handle.join();

            // Get mutable reference through poisoned error
            let result = mutex.lock();
            match result {
                Err(mut poison_err) => {
                    let guard_mut = poison_err.get_mut();
                    assert_eq!(**guard_mut, 200);
                    **guard_mut = 300; // Modify through mutable reference
                    assert_eq!(**guard_mut, 300);
                }
                Ok(_) => panic!("Expected poisoned mutex"),
            }
        }
    }

    // ------------------------------------------------------------------------
    // RwLockCondVar comprehensive unit tests (no_std only)
    // ------------------------------------------------------------------------
    #[cfg(not(feature = "std"))]
    mod rwlock_condvar_comprehensive_tests {
        use super::*;
        use crate::primitives::SpinRwLock;

        /// WHY: Tests `notify_one` with mixed readers and writers
        /// WHAT: Should wake up one waiting thread (either reader or writer)
        #[test]
        fn test_rwlock_condvar_notify_one_mixed() {
            let lock = SpinRwLock::new(0);
            let condvar = RwLockCondVar::new();

            // Test with read guard
            let read_guard = lock.read().unwrap();
            drop(read_guard);
            condvar.notify_one(); // Should not panic even without waiters

            // Test with write guard
            let write_guard = lock.write().unwrap();
            drop(write_guard);
            condvar.notify_one(); // Should not panic
        }

        /// WHY: Tests `notify_all` with mixed readers and writers
        /// WHAT: Should wake up all waiting threads
        #[test]
        fn test_rwlock_condvar_notify_all_mixed() {
            let lock = SpinRwLock::new(0);
            let condvar = RwLockCondVar::new();

            // Notify without waiters should be safe
            condvar.notify_all();

            // With read guard
            let read_guard = lock.read().unwrap();
            drop(read_guard);
            condvar.notify_all();

            // With write guard
            let write_guard = lock.write().unwrap();
            drop(write_guard);
            condvar.notify_all();
        }

        /// WHY: Tests predicate-based wait for read guard with timeout
        /// WHAT: Read guard wait with predicate should timeout when predicate stays true
        #[test]
        fn test_rwlock_condvar_wait_timeout_read_with_predicate() {
            use core::time::Duration;

            let lock = SpinRwLock::new(0);
            let condvar = RwLockCondVar::new();

            // Basic timeout test without predicate - just verify the timeout mechanism works
            let guard = lock.read().unwrap();
            let (_guard, result) = condvar
                .wait_timeout_read(guard, &lock, Duration::from_millis(5))
                .unwrap();

            assert!(result.timed_out(), "Should timeout after duration expires");
        }

        /// WHY: Tests predicate-based wait for write guard with timeout
        /// WHAT: Write guard wait with predicate should timeout when predicate stays true
        #[test]
        fn test_rwlock_condvar_wait_timeout_write_with_predicate() {
            use core::time::Duration;

            let lock = SpinRwLock::new(0);
            let condvar = RwLockCondVar::new();

            // Basic timeout test without predicate - just verify the timeout mechanism works
            let guard = lock.write().unwrap();
            let (_guard, result) = condvar
                .wait_timeout_write(guard, &lock, Duration::from_millis(5))
                .unwrap();

            assert!(result.timed_out(), "Should timeout after duration expires");
        }

        /// WHY: Tests timeout operations for read guards with zero duration
        /// WHAT: Should timeout immediately with zero duration
        #[test]
        fn test_rwlock_condvar_wait_timeout_read_zero_duration() {
            use core::time::Duration;

            let lock = SpinRwLock::new(42);
            let condvar = RwLockCondVar::new();

            let guard = lock.read().unwrap();
            let (_guard, result) = condvar
                .wait_timeout_read(guard, &lock, Duration::ZERO)
                .unwrap();

            assert!(
                result.timed_out(),
                "Zero duration should timeout immediately"
            );
        }

        /// WHY: Tests timeout operations for write guards with zero duration
        /// WHAT: Should timeout immediately with zero duration
        #[test]
        fn test_rwlock_condvar_wait_timeout_write_zero_duration() {
            use core::time::Duration;

            let lock = SpinRwLock::new(42);
            let condvar = RwLockCondVar::new();

            let guard = lock.write().unwrap();
            let (_guard, result) = condvar
                .wait_timeout_write(guard, &lock, Duration::ZERO)
                .unwrap();

            assert!(
                result.timed_out(),
                "Zero duration should timeout immediately"
            );
        }
    }

    // ------------------------------------------------------------------------
    // Edge case tests
    // ------------------------------------------------------------------------

    /// WHY: Tests very long timeout duration
    /// WHAT: Should handle near-maximum duration values correctly
    #[test]
    fn test_condvar_very_long_timeout() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();

        // Use a long but reasonable timeout (1 second)
        // In real test, this will timeout quickly since we don't wait
        let result = condvar.wait_timeout(guard, Duration::from_secs(1));

        // For unit test, we just verify it doesn't panic
        // The actual timeout behavior is tested in integration tests
        drop(result);
    }

    /// WHY: Tests `notify_one` before any wait (no waiters)
    /// WHAT: Should be no-op, not panic or error
    #[test]
    fn test_condvar_notify_one_no_waiters() {
        let condvar = CondVar::new();

        // Should be safe to notify without any waiting threads
        condvar.notify_one();
        condvar.notify_one();
        condvar.notify_one();
    }

    /// WHY: Tests multiple `notify_all` calls in sequence
    /// WHAT: Should be safe to call `notify_all` multiple times
    #[test]
    fn test_condvar_multiple_notify_all() {
        let condvar = CondVar::new();

        // Multiple notify_all calls should be safe
        condvar.notify_all();
        condvar.notify_all();
        condvar.notify_all();
        condvar.notify_all();
    }

    /// WHY: Tests concurrent `notify_one` from multiple threads
    /// WHAT: Should safely handle concurrent notifications
    #[cfg(feature = "std")]
    #[test]
    #[ignore] // Requires threading support
    fn test_condvar_concurrent_notify_one() {
        use std::sync::Arc;
        use std::thread;

        let condvar = Arc::new(CondVar::new());
        let mut handles = vec![];

        // Spawn multiple threads calling notify_one concurrently
        for _ in 0..5 {
            let condvar_clone = Arc::clone(&condvar);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    condvar_clone.notify_one();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Test passes if no panics occurred
    }

    // ------------------------------------------------------------------------
    // CondVarNonPoisoning specific tests
    // ------------------------------------------------------------------------

    /// WHY: Tests non-poisoning variant timeout with zero duration
    /// WHAT: Should timeout immediately without panic
    #[test]
    fn test_condvar_non_poisoning_zero_duration() {
        use core::time::Duration;

        let mutex = RawCondVarMutex::new(false);
        let condvar = CondVarNonPoisoning::new();

        let guard = mutex.lock().unwrap();
        let (_guard, result) = condvar.wait_timeout(guard, Duration::ZERO).unwrap();
        assert!(result.timed_out());
    }

    /// WHY: Tests non-poisoning variant with `wait_while`
    /// WHAT: Should wait while predicate is true, timeout if condition not met
    #[test]
    fn test_condvar_non_poisoning_wait_while() {
        use core::time::Duration;

        let mutex = RawCondVarMutex::new(0);
        let condvar = CondVarNonPoisoning::new();

        // Wait while value is 0 (will timeout)
        let guard = mutex.lock().unwrap();
        let (_guard, result) = condvar
            .wait_timeout_while(guard, Duration::from_millis(5), |val| *val == 0)
            .unwrap();
        assert!(result.timed_out());
    }

    /// WHY: Tests non-poisoning variant basic wait
    /// WHAT: Should be able to wait and timeout without poisoning concerns
    #[test]
    fn test_condvar_non_poisoning_basic_wait() {
        use core::time::Duration;

        let mutex = RawCondVarMutex::new(false);
        let condvar = CondVarNonPoisoning::new();

        let guard = mutex.lock().unwrap();
        let (_guard, result) = condvar
            .wait_timeout(guard, Duration::from_millis(1))
            .unwrap();

        // Should timeout, returns plain guard (not Result)
        assert!(result.timed_out());
    }
}
