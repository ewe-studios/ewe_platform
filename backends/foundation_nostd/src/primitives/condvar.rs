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
    #[must_use]
    pub const fn new(timed_out: bool) -> Self {
        Self(timed_out)
    }
}

// Feature-gate the implementation
// js-event-loop always uses nostd_impl (spin-waiting) since std::thread::sleep panics on wasm32
#[cfg(all(feature = "std", not(feature = "js-event-loop")))]
mod std_impl;
#[cfg(all(feature = "std", not(feature = "js-event-loop")))]
pub use std_impl::{
    CondVar, CondVarMutex, CondVarMutexGuard, CondVarNonPoisoning, RawCondVarMutex,
    RawCondVarMutexGuard, RwLockCondVar,
};

#[cfg(any(not(feature = "std"), feature = "js-event-loop"))]
mod nostd_impl;
#[cfg(any(not(feature = "std"), feature = "js-event-loop"))]
pub use nostd_impl::{
    CondVar, CondVarMutex, CondVarMutexGuard, CondVarNonPoisoning, RawCondVarMutex,
    RawCondVarMutexGuard, RwLockCondVar,
};
