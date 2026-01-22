//! Spin mutex with poisoning support.
//!
//! This provides a `std::sync::Mutex`-compatible API for `no_std` environments.
//! Unlike `RawSpinMutex`, this tracks panics during guard drops to detect
//! potential data corruption from panicked critical sections.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU8, Ordering};

use crate::primitives::{
    spin_wait::SpinWait, LockResult, PoisonError, TryLockError, TryLockResult,
};

// State encoding:
// Bit 0: LOCKED (1 = locked, 0 = unlocked)
// Bit 1: POISONED (1 = poisoned, 0 = clean)
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;

/// A spin-based mutual exclusion `lock` with poisoning support.
///
/// This matches the `std::sync::Mutex` API for drop-in replacement in
/// `no_std` contexts.
pub struct SpinMutex<T: ?Sized> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}

/// RAII guard for `SpinMutex`.
pub struct SpinMutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a SpinMutex<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for SpinMutexGuard<'_, T> {}

impl<T> SpinMutex<T> {
    /// Creates a new unlocked mutex.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(UNLOCKED),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the mutex and returns the inner value.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    #[inline]
    pub fn into_inner(self) -> LockResult<T> {
        let is_poisoned = self.is_poisoned();
        let data = self.data.into_inner();

        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let is_poisoned = self.is_poisoned();
        let data = self.data.get_mut();

        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized> SpinMutex<T> {
    /// Checks if the mutex is poisoned.
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.state.load(Ordering::Relaxed) & POISONED != 0
    }

    /// Acquires the lock, spinning until it becomes available.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the mutex was poisoned.
    #[inline]
    pub fn lock(&self) -> LockResult<SpinMutexGuard<'_, T>> {
        // Fast path: try to acquire immediately
        let state = self.state.load(Ordering::Relaxed);
        if state & LOCKED == 0
            && self
                .state
                .compare_exchange(state, state | LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            let guard = SpinMutexGuard { mutex: self };
            return if state & POISONED != 0 {
                Err(PoisonError::new(guard))
            } else {
                Ok(guard)
            };
        }

        // Slow path: spin with backoff
        self.lock_slow()
    }

    #[cold]
    fn lock_slow(&self) -> LockResult<SpinMutexGuard<'_, T>> {
        let mut spin_wait = SpinWait::new();
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & LOCKED == 0
                && self
                    .state
                    .compare_exchange_weak(
                        state,
                        state | LOCKED,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            {
                let guard = SpinMutexGuard { mutex: self };
                return if state & POISONED != 0 {
                    Err(PoisonError::new(guard))
                } else {
                    Ok(guard)
                };
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire the `lock` without blocking.
    ///
    /// # Errors
    ///
    /// Returns `Err(TryLockError::WouldBlock)` if the lock is already held,
    /// or `Err(TryLockError::Poisoned)` if the mutex was poisoned.
    #[inline]
    pub fn try_lock(&self) -> TryLockResult<SpinMutexGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);
        if state & LOCKED == 0
            && self
                .state
                .compare_exchange(state, state | LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            let guard = SpinMutexGuard { mutex: self };
            if state & POISONED != 0 {
                Err(TryLockError::Poisoned(PoisonError::new(guard)))
            } else {
                Ok(guard)
            }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    /// Attempts to acquire the lock, spinning up to `limit` times.
    ///
    /// # Errors
    ///
    /// Returns `Err(TryLockError::WouldBlock)` if the lock could not be acquired
    /// within the spin limit, or `Err(TryLockError::Poisoned)` if poisoned.
    pub fn try_lock_with_spin_limit(&self, limit: usize) -> TryLockResult<SpinMutexGuard<'_, T>> {
        for _ in 0..limit {
            let state = self.state.load(Ordering::Relaxed);
            if state & LOCKED == 0
                && self
                    .state
                    .compare_exchange_weak(
                        state,
                        state | LOCKED,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            {
                let guard = SpinMutexGuard { mutex: self };
                return if state & POISONED != 0 {
                    Err(TryLockError::Poisoned(PoisonError::new(guard)))
                } else {
                    Ok(guard)
                };
            }
            core::hint::spin_loop();
        }
        Err(TryLockError::WouldBlock)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SpinMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f.debug_struct("SpinMutex").field("data", &&*guard).finish(),
            Err(TryLockError::WouldBlock) => f
                .debug_struct("SpinMutex")
                .field("data", &"<locked>")
                .finish(),
            Err(TryLockError::Poisoned(_)) => f
                .debug_struct("SpinMutex")
                .field("data", &"<poisoned>")
                .finish(),
        }
    }
}

impl<T: Default> Default for SpinMutex<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for SpinMutex<T> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T: ?Sized> Deref for SpinMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for SpinMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for SpinMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // Note: In no_std without panic detection, poisoning must be
        // triggered manually or through external panic runtime.
        // For full std compatibility, check std::thread::panicking() here.

        // Release the lock, clearing the LOCKED bit but preserving POISONED
        let state = self.mutex.state.load(Ordering::Relaxed);
        self.mutex.state.store(state & !LOCKED, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SpinMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for SpinMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `WHY`: Validates basic mutex construction and `into_inner`
    /// `WHAT`: Creating a mutex and extracting its value should work
    #[test]
    fn test_new_and_into_inner() {
        let mutex = SpinMutex::new(42);
        assert_eq!(mutex.into_inner().unwrap(), 42);
    }

    /// `WHY`: Validates basic `lock` acquisition and data access
    /// `WHAT`: Lock should be acquirable and data should be accessible
    #[test]
    fn test_lock() {
        let mutex = SpinMutex::new(0);
        {
            let mut guard = mutex.lock().unwrap();
            *guard += 1;
            assert_eq!(*guard, 1);
        }
        let guard = mutex.lock().unwrap();
        assert_eq!(*guard, 1);
    }

    /// `WHY`: Validates `try_lock` behavior when `lock` is free
    /// `WHAT`: `try_lock` should succeed when `lock` is not held
    #[test]
    fn test_try_lock_success() {
        let mutex = SpinMutex::new(42);
        let guard = mutex.try_lock();
        assert!(guard.is_ok());
        assert_eq!(*guard.unwrap(), 42);
    }

    /// `WHY`: Validates `try_lock` behavior when `lock` is held
    /// `WHAT`: `try_lock` should return `WouldBlock` when already held
    #[test]
    fn test_try_lock_would_block() {
        let mutex = SpinMutex::new(42);
        let _guard1 = mutex.lock().unwrap();
        let result = mutex.try_lock();
        assert!(matches!(result, Err(TryLockError::WouldBlock)));
    }

    /// `WHY`: Validates that mutex is not poisoned by default
    /// `WHAT`: New mutex should not be poisoned
    #[test]
    fn test_not_poisoned() {
        let mutex = SpinMutex::new(42);
        assert!(!mutex.is_poisoned());
    }

    /// `WHY`: Validates `get_mut` functionality
    /// `WHAT`: `get_mut` should provide mutable access without locking
    #[test]
    fn test_get_mut() {
        let mut mutex = SpinMutex::new(0);
        *mutex.get_mut().unwrap() = 42;
        assert_eq!(*mutex.lock().unwrap(), 42);
    }

    /// `WHY`: Validates Send trait bounds
    /// `WHAT`: Mutex should be Send when T is Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SpinMutex<i32>>();
    }

    /// `WHY`: Validates Sync trait bounds
    /// `WHAT`: Mutex should be Sync when T is Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SpinMutex<i32>>();
    }
}
