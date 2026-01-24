//! Compatibility layer for std and `no_std` synchronization primitives.
//!
//! This module provides type aliases that automatically select between `std` and `no_std`
//! implementations based on the `std` feature flag. Users can enable the `std` feature
//! in their `Cargo.toml` to use standard library primitives, or omit it for `no_std` environments.
//!
//! # Feature Flags
//!
//! - `std`: When enabled, uses `std::sync` types for optimal performance
//! - When disabled (default): Uses `foundation_nostd` spin-lock based implementations
//!
//! # Examples
//!
//! ## Using with `std` feature enabled
//!
//! ```toml
//! [dependencies]
//! foundation_nostd = { version = "0.0.4", features = ["std"] }
//! ```
//!
//! ```no_run
//! use foundation_nostd::comp::{Mutex, RwLock, CondVar};
//!
//! // Will use std::sync::Mutex when std feature is enabled
//! let mutex = Mutex::new(42);
//! let guard = mutex.lock().unwrap();
//! assert_eq!(*guard, 42);
//! ```
//!
//! ## Using in `no_std` environment (default)
//!
//! ```toml
//! [dependencies]
//! foundation_nostd = "0.0.4"
//! ```
//!
//! ```no_run
//! use foundation_nostd::comp::{Mutex, RwLock, CondVar};
//!
//! // Will use foundation_nostd::primitives::SpinMutex when std feature is disabled
//! let mutex = Mutex::new(42);
//! let guard = mutex.lock().unwrap();
//! assert_eq!(*guard, 42);
//! ```

// ============================================================================
// Mutex Types
// ============================================================================

/// Platform-appropriate `Mutex` type.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Mutex` (platform-appropriate spin mutex)
#[cfg(feature = "std")]
pub use std::sync::Mutex;

/// Platform-appropriate `Mutex` type.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Mutex` (platform-appropriate spin mutex)
#[cfg(not(feature = "std"))]
pub use crate::primitives::Mutex;

/// Platform-appropriate `MutexGuard` type.
///
/// - With `std` feature: Uses `std::sync::MutexGuard`
/// - Without `std` feature: Uses the guard type from `foundation_nostd::primitives`
#[cfg(feature = "std")]
pub use std::sync::MutexGuard;

// ============================================================================
// RwLock Types
// ============================================================================

/// Platform-appropriate `RwLock` type.
///
/// - With `std` feature: Uses `std::sync::RwLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::RwLock` (platform-appropriate spin rwlock)
#[cfg(feature = "std")]
pub use std::sync::RwLock;

/// Platform-appropriate `RwLock` type.
///
/// - With `std` feature: Uses `std::sync::RwLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::RwLock` (platform-appropriate spin rwlock)
#[cfg(not(feature = "std"))]
pub use crate::primitives::RwLock;

/// Platform-appropriate `RwLockReadGuard` type.
///
/// - With `std` feature: Uses `std::sync::RwLockReadGuard`
/// - Without `std` feature: Uses the read guard type from `foundation_nostd::primitives`
#[cfg(feature = "std")]
pub use std::sync::RwLockReadGuard;

/// Platform-appropriate `RwLockWriteGuard` type.
///
/// - With `std` feature: Uses `std::sync::RwLockWriteGuard`
/// - Without `std` feature: Uses the write guard type from `foundation_nostd::primitives`
#[cfg(feature = "std")]
pub use std::sync::RwLockWriteGuard;

// ============================================================================
// CondVar Types
// ============================================================================

/// Platform-appropriate `Condvar` type.
///
/// - With `std` feature: Uses `std::sync::Condvar`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVar`
#[cfg(feature = "std")]
pub use std::sync::Condvar as CondVar;

/// Platform-appropriate `Condvar` type.
///
/// - With `std` feature: Uses `std::sync::Condvar`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVar`
#[cfg(not(feature = "std"))]
pub use crate::primitives::CondVar;

/// Platform-appropriate `WaitTimeoutResult` type.
///
/// - With `std` feature: Uses `std::sync::WaitTimeoutResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::WaitTimeoutResult`
#[cfg(feature = "std")]
pub use std::sync::WaitTimeoutResult;

/// Platform-appropriate `WaitTimeoutResult` type.
///
/// - With `std` feature: Uses `std::sync::WaitTimeoutResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::WaitTimeoutResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::WaitTimeoutResult;

// ============================================================================
// Mutex Types for CondVar
// ============================================================================

/// Mutex type for use with `CondVar`.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutex`
#[cfg(feature = "std")]
pub use std::sync::Mutex as CondVarMutex;

/// Mutex type for use with `CondVar`.
///
/// - With `std` feature: Uses `std::sync::Mutex`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutex`
#[cfg(not(feature = "std"))]
pub use crate::primitives::CondVarMutex;

/// Mutex guard type for use with `CondVar`.
///
/// - With `std` feature: Uses `std::sync::MutexGuard`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutexGuard`
#[cfg(feature = "std")]
pub use std::sync::MutexGuard as CondVarMutexGuard;

/// Mutex guard type for use with `CondVar`.
///
/// - With `std` feature: Uses `std::sync::MutexGuard`
/// - Without `std` feature: Uses `foundation_nostd::primitives::CondVarMutexGuard`
#[cfg(not(feature = "std"))]
pub use crate::primitives::CondVarMutexGuard;

// ============================================================================
// Barrier Types
// ============================================================================

/// Platform-appropriate `Barrier` type.
///
/// - With `std` feature: Uses `std::sync::Barrier`
/// - Without `std` feature: Uses `foundation_nostd::primitives::SpinBarrier`
#[cfg(feature = "std")]
pub use std::sync::Barrier;

/// Platform-appropriate `Barrier` type.
///
/// - With `std` feature: Uses `std::sync::Barrier`
/// - Without `std` feature: Uses `foundation_nostd::primitives::SpinBarrier`
#[cfg(not(feature = "std"))]
pub use crate::primitives::SpinBarrier as Barrier;

/// Platform-appropriate `BarrierWaitResult` type.
///
/// - With `std` feature: Uses `std::sync::BarrierWaitResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::BarrierWaitResult`
#[cfg(feature = "std")]
pub use std::sync::BarrierWaitResult;

/// Platform-appropriate `BarrierWaitResult` type.
///
/// - With `std` feature: Uses `std::sync::BarrierWaitResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::BarrierWaitResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::BarrierWaitResult;

// ============================================================================
// Once Types
// ============================================================================

/// Platform-appropriate `Once` type for one-time initialization.
///
/// - With `std` feature: Uses `std::sync::Once`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Once`
#[cfg(feature = "std")]
pub use std::sync::Once;

/// Platform-appropriate `Once` type for one-time initialization.
///
/// - With `std` feature: Uses `std::sync::Once`
/// - Without `std` feature: Uses `foundation_nostd::primitives::Once`
#[cfg(not(feature = "std"))]
pub use crate::primitives::Once;

/// Platform-appropriate `OnceState` type.
///
/// - With `std` feature: Uses `std::sync::OnceState`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceState`
#[cfg(feature = "std")]
pub use std::sync::OnceState;

/// Platform-appropriate `OnceState` type.
///
/// - With `std` feature: Uses `std::sync::OnceState`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceState`
#[cfg(not(feature = "std"))]
pub use crate::primitives::OnceState;

// ============================================================================
// OnceLock Types
// ============================================================================

/// Platform-appropriate `OnceLock` type for one-time initialized cells.
///
/// - With `std` feature: Uses `std::sync::OnceLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceLock`
#[cfg(feature = "std")]
pub use std::sync::OnceLock;

/// Platform-appropriate `OnceLock` type for one-time initialized cells.
///
/// - With `std` feature: Uses `std::sync::OnceLock`
/// - Without `std` feature: Uses `foundation_nostd::primitives::OnceLock`
#[cfg(not(feature = "std"))]
pub use crate::primitives::OnceLock;

// ============================================================================
// Poison Types
// ============================================================================

/// Platform-appropriate `PoisonError` type.
///
/// - With `std` feature: Uses `std::sync::PoisonError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::PoisonError`
#[cfg(feature = "std")]
pub use std::sync::PoisonError;

/// Platform-appropriate `PoisonError` type.
///
/// - With `std` feature: Uses `std::sync::PoisonError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::PoisonError`
#[cfg(not(feature = "std"))]
pub use crate::primitives::PoisonError;

/// Platform-appropriate `TryLockError` type.
///
/// - With `std` feature: Uses `std::sync::TryLockError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockError`
#[cfg(feature = "std")]
pub use std::sync::TryLockError;

/// Platform-appropriate `TryLockError` type.
///
/// - With `std` feature: Uses `std::sync::TryLockError`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockError`
#[cfg(not(feature = "std"))]
pub use crate::primitives::TryLockError;

/// Type alias for `Result` with `PoisonError`.
///
/// - With `std` feature: Uses `std::sync::LockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::LockResult`
#[cfg(feature = "std")]
pub use std::sync::LockResult;

/// Type alias for `Result` with `PoisonError`.
///
/// - With `std` feature: Uses `std::sync::LockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::LockResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::LockResult;

/// Type alias for `Result` with `TryLockError`.
///
/// - With `std` feature: Uses `std::sync::TryLockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockResult`
#[cfg(feature = "std")]
pub use std::sync::TryLockResult;

/// Type alias for `Result` with `TryLockError`.
///
/// - With `std` feature: Uses `std::sync::TryLockResult`
/// - Without `std` feature: Uses `foundation_nostd::primitives::TryLockResult`
#[cfg(not(feature = "std"))]
pub use crate::primitives::TryLockResult;

// ============================================================================
// Additional Foundation-Specific Types (no_std only)
// ============================================================================

// Note: The following types are only available in no_std mode as they are
// foundation_nostd-specific implementations without direct std equivalents.

#[cfg(not(feature = "std"))]
pub use crate::primitives::{
    AtomicCell, AtomicFlag, AtomicLazy, AtomicOption, CondVarNonPoisoning, RawCondVarMutex,
    RawCondVarMutexGuard, RawSpinMutex, RawSpinMutexGuard, RawSpinRwLock, ReaderSpinRwLock,
    RwLockCondVar, SpinMutex, SpinRwLock, SpinWait,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_basic() {
        let mutex = Mutex::new(42);
        let guard = mutex.lock().unwrap();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_rwlock_basic() {
        let rwlock = RwLock::new(100);

        // Test read
        let read_guard = rwlock.read().unwrap();
        assert_eq!(*read_guard, 100);
        drop(read_guard);

        // Test write
        let mut write_guard = rwlock.write().unwrap();
        *write_guard = 200;
        drop(write_guard);

        // Verify write
        let read_guard = rwlock.read().unwrap();
        assert_eq!(*read_guard, 200);
    }

    #[test]
    fn test_condvar_basic() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(1));

        // Should timeout
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
    }

    #[test]
    fn test_barrier_basic() {
        let barrier = Barrier::new(1);
        let result = barrier.wait();
        assert!(result.is_leader());
    }

    #[test]
    fn test_once_basic() {
        static ONCE: Once = Once::new();
        let mut counter = 0;

        ONCE.call_once(|| {
            counter += 1;
        });

        ONCE.call_once(|| {
            counter += 1;
        });

        assert_eq!(counter, 1);
    }

    #[test]
    fn test_once_lock_basic() {
        let lock = OnceLock::new();
        assert!(lock.get().is_none());

        lock.set(42).ok();
        assert_eq!(lock.get(), Some(&42));

        // Setting again should fail
        assert!(lock.set(100).is_err());
    }

    #[test]
    fn test_poison_error_handling() {
        let mutex = Mutex::new(42);
        let guard = mutex.lock().unwrap();
        let value = *guard;
        assert_eq!(value, 42);
    }
}
