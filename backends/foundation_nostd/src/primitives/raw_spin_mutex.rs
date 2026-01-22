//! Raw spin mutex without poisoning support.
//!
//! This provides the simplest possible spin-lock implementation for `no_std`
//! environments where poisoning is unnecessary (e.g., panic = abort).
//!
//! # Examples
//!
//! ```
//! use foundation_nostd::primitives::RawSpinMutex;
//!
//! let mutex = RawSpinMutex::new(0);
//!
//! {
//!     let mut guard = mutex.lock();
//!     *guard += 1;
//! } // Lock released here
//!
//! // Try to acquire without blocking
//! if let Some(guard) = mutex.try_lock() {
//!     println!("Got lock: {}", *guard);
//! }
//! ```

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::primitives::spin_wait::SpinWait;

/// A spin-based mutual exclusion lock without poisoning support.
///
/// This is simpler than `SpinMutex` and suitable for embedded systems where
/// panic = abort (no unwinding), making poisoning unnecessary.
pub struct RawSpinMutex<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RawSpinMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for RawSpinMutex<T> {}

/// RAII guard for `RawSpinMutex`.
pub struct RawSpinMutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a RawSpinMutex<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for RawSpinMutexGuard<'_, T> {}

impl<T> RawSpinMutex<T> {
    /// Creates a new unlocked mutex.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the mutex and returns the inner value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    /// Returns a mutable reference to the underlying data.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: ?Sized> RawSpinMutex<T> {
    /// Acquires the lock, spinning until it becomes available.
    #[inline]
    pub fn lock(&self) -> RawSpinMutexGuard<'_, T> {
        // Fast path: try to acquire immediately
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return RawSpinMutexGuard { mutex: self };
        }

        // Slow path: spin with backoff
        self.lock_slow()
    }

    #[cold]
    fn lock_slow(&self) -> RawSpinMutexGuard<'_, T> {
        let mut spin_wait = SpinWait::new();
        loop {
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return RawSpinMutexGuard { mutex: self };
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire the lock without blocking.
    #[inline]
    pub fn try_lock(&self) -> Option<RawSpinMutexGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RawSpinMutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Attempts to acquire the lock, spinning up to `limit` times.
    pub fn try_lock_with_spin_limit(&self, limit: usize) -> Option<RawSpinMutexGuard<'_, T>> {
        for _ in 0..limit {
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Some(RawSpinMutexGuard { mutex: self });
            }
            core::hint::spin_loop();
        }
        None
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawSpinMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f
                .debug_struct("RawSpinMutex")
                .field("data", &&*guard)
                .finish(),
            None => f
                .debug_struct("RawSpinMutex")
                .field("data", &"<locked>")
                .finish(),
        }
    }
}

impl<T: Default> Default for RawSpinMutex<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for RawSpinMutex<T> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> Deref for RawSpinMutexGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for RawSpinMutexGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for RawSpinMutexGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawSpinMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RawSpinMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Validates basic mutex construction and into_inner
    /// WHAT: Creating a mutex and extracting its value should work
    #[test]
    fn test_new_and_into_inner() {
        let mutex = RawSpinMutex::new(42);
        assert_eq!(mutex.into_inner(), 42);
    }

    /// WHY: Validates basic lock acquisition and data access
    /// WHAT: Lock should be acquirable and data should be accessible
    #[test]
    fn test_lock() {
        let mutex = RawSpinMutex::new(0);
        {
            let mut guard = mutex.lock();
            *guard += 1;
            assert_eq!(*guard, 1);
        }
        let guard = mutex.lock();
        assert_eq!(*guard, 1);
    }

    /// WHY: Validates try_lock behavior when lock is free
    /// WHAT: try_lock should succeed when lock is not held
    #[test]
    fn test_try_lock_success() {
        let mutex = RawSpinMutex::new(42);
        let guard = mutex.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.unwrap(), 42);
    }

    /// WHY: Validates try_lock behavior when lock is held
    /// WHAT: try_lock should fail when lock is already held
    #[test]
    fn test_try_lock_failure() {
        let mutex = RawSpinMutex::new(42);
        let _guard1 = mutex.lock();
        let guard2 = mutex.try_lock();
        assert!(guard2.is_none());
    }

    /// WHY: Validates try_lock_with_spin_limit functionality
    /// WHAT: Should succeed within limit when lock becomes available
    #[test]
    fn test_try_lock_with_spin_limit() {
        let mutex = RawSpinMutex::new(42);

        // Should succeed immediately when lock is free
        let guard = mutex.try_lock_with_spin_limit(1000);
        assert!(guard.is_some());
        drop(guard);

        // Should fail within limit when lock is held
        let _guard1 = mutex.lock();
        let guard2 = mutex.try_lock_with_spin_limit(10);
        assert!(guard2.is_none());
    }

    /// WHY: Validates get_mut functionality with exclusive borrow
    /// WHAT: get_mut should provide mutable access without locking
    #[test]
    fn test_get_mut() {
        let mut mutex = RawSpinMutex::new(0);
        *mutex.get_mut() = 42;
        assert_eq!(*mutex.lock(), 42);
    }

    /// WHY: Validates Send trait bounds
    /// WHAT: Mutex should be Send when T is Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RawSpinMutex<i32>>();
    }

    /// WHY: Validates Sync trait bounds
    /// WHAT: Mutex should be Sync when T is Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RawSpinMutex<i32>>();
    }
}
