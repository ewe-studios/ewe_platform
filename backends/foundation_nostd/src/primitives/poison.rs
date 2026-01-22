//! Poison error types for synchronization primitives.
//!
//! This module provides error types that match `std::sync` poisoning behavior.
//! When a thread panics while holding a lock, the lock becomes "poisoned" to
//! detect potential data corruption from panicked critical sections.

use core::error::Error;
use core::fmt;

/// A type alias for the result of a `lock` method which can be poisoned.
///
/// This is identical to `std::sync::LockResult`.
pub type LockResult<Guard> = Result<Guard, PoisonError<Guard>>;

/// A type alias for the result of a nonblocking `lock` method which can be poisoned.
///
/// This is identical to `std::sync::TryLockResult`.
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

/// A poison error which can be returned from locks.
///
/// When a thread panics while holding a lock, the `lock` is poisoned to signal
/// that the protected data may be in an inconsistent state. This error wraps
/// the guard to allow recovery by explicitly acknowledging the poisoned state.
///
/// # Examples
///
/// ```ignore
/// use foundation_nostd::primitives::{SpinMutex, `PoisonError`};
///
/// let mutex = SpinMutex::new(0);
/// let result = mutex.lock();
///
/// match result {
///     Ok(guard) => {
///         // Normal case: `lock` acquired, data is consistent
///         assert_eq!(*guard, 0);
///     }
///     Err(poisoned) => {
///         // Lock was poisoned by a panicked thread
///         // Can recover by calling `into_inner`()
///         let guard = poisoned.into_inner();
///         // Now we can access the data despite poisoning
///     }
/// }
/// ```ignore
#[derive(Debug)]
pub struct PoisonError<T> {
    guard: T,
}

impl<T> PoisonError<T> {
    /// Creates a new `PoisonError`.
    #[inline]
    pub fn new(guard: T) -> Self {
        Self { guard }
    }

    /// Consumes this error, returning the underlying guard.
    ///
    /// # Errors
    ///
    /// This function returns a `Result` for API compatibility but never fails.
    ///
    /// This allows access to the protected data despite the poisoned state.
    #[inline]
    pub fn into_inner(self) -> T {
        self.guard
    }

    /// Gets a reference to the underlying guard.
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.guard
    }

    /// Gets a mutable reference to the underlying guard.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T> fmt::Display for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "poisoned lock: another thread panicked while holding this lock"
        )
    }
}

impl<T: fmt::Debug> Error for PoisonError<T> {}

/// An enumeration of possible errors from the `try_lock` method.
///
/// This is identical to `std::sync::TryLockError`.
#[derive(Debug)]
pub enum TryLockError<T> {
    /// The `lock` could not be acquired because another thread is holding it.
    WouldBlock,

    /// The `lock` was poisoned (a thread panicked while holding it).
    ///
    /// The wrapped `PoisonError` contains the guard, allowing recovery.
    Poisoned(PoisonError<T>),
}

impl<T> fmt::Display for TryLockError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WouldBlock => write!(f, "try_lock failed because the lock was already held"),
            Self::Poisoned(err) => write!(f, "{err}"),
        }
    }
}

impl<T: fmt::Debug> Error for TryLockError<T> {}

impl<T> From<PoisonError<T>> for TryLockError<T> {
    #[inline]
    fn from(err: PoisonError<T>) -> Self {
        Self::Poisoned(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    /// `WHY`: Validates `PoisonError` construction and `into_inner` recovery
    /// `WHAT`: Creating a `PoisonError` should allow extracting the wrapped value
    #[test]
    fn test_poison_error_into_inner() {
        let guard = 42;
        let error = PoisonError::new(guard);
        assert_eq!(error.into_inner(), 42);
    }

    /// `WHY`: Validates `PoisonError` reference access methods
    /// `WHAT`: `get_ref` and `get_mut` should provide access to the wrapped guard
    #[test]
    fn test_poison_error_get_ref() {
        let mut error = PoisonError::new(42);
        assert_eq!(*error.get_ref(), 42);
        *error.get_mut() = 100;
        assert_eq!(*error.get_ref(), 100);
    }

    /// `WHY`: Validates `TryLockError` enum variants and conversions
    /// `WHAT`: `TryLockError` should support both `WouldBlock` and Poisoned variants
    #[test]
    fn test_try_lock_error_variants() {
        let would_block: TryLockError<i32> = TryLockError::WouldBlock;
        assert!(matches!(would_block, TryLockError::WouldBlock));

        let poison = PoisonError::new(42);
        let poisoned: TryLockError<i32> = TryLockError::from(poison);
        assert!(matches!(poisoned, TryLockError::Poisoned(_)));
    }

    /// `WHY`: Validates error messages for user-facing display
    /// `WHAT`: Display trait should provide meaningful error messages
    #[test]
    fn test_error_display() {
        let poison = PoisonError::new(42);
        let msg = format!("{poison}");
        assert!(msg.contains("poisoned"));

        let would_block: TryLockError<i32> = TryLockError::WouldBlock;
        let msg = format!("{would_block}");
        assert!(msg.contains("already held") || msg.contains("try_lock failed"));
    }
}
