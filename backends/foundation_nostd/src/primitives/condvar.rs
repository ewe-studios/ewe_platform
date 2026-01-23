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
}
