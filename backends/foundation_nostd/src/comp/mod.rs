//! Compatibility layer for std and `no_std` synchronization primitives.
//!
//! This module provides type aliases that automatically select between `std` and `no_std`
//! implementations based on the `std` feature flag. Users can enable the `std` feature
//! in their `Cargo.toml` to use standard library primitives, or omit it for `no_std` environments.
//!
//! # Submodules
//!
//! - [`basic`]: Basic sync primitives (Mutex, RwLock, Barrier, Once, OnceLock)
//! - [`condvar_comp`]: CondVar with properly paired Mutex types
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
//! use foundation_nostd::comp::basic::{Mutex, RwLock};
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
//! use foundation_nostd::comp::basic::{Mutex, RwLock};
//!
//! // Will use foundation_nostd::primitives::SpinMutex when std feature is disabled
//! let mutex = Mutex::new(42);
//! let guard = mutex.lock().unwrap();
//! assert_eq!(*guard, 42);
//! ```
//!
//! ## Using CondVar with proper Mutex pairing
//!
//! ```no_run
//! use foundation_nostd::comp::condvar_comp::{Mutex, CondVar};
//!
//! let mutex = Mutex::new(false);
//! let condvar = CondVar::new();
//!
//! let mut guard = mutex.lock().unwrap();
//! while !*guard {
//!     guard = condvar.wait(guard).unwrap();
//! }
//! ```

pub mod basic;
pub mod condvar_comp;

#[cfg(test)]
mod tests {
    use super::basic::{Barrier, Mutex, Once, OnceLock, RwLock};
    use super::condvar_comp::{CondVar, Mutex as CondVarMutex};

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
