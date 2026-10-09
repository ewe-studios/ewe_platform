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

/// A spin-based mutual exclusion `lock` without poisoning support.
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

    /// Attempts to acquire the `lock` without blocking.
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

impl<T: ?Sized> Deref for RawSpinMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RawSpinMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for RawSpinMutexGuard<'_, T> {
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
