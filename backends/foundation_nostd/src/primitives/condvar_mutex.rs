//! CondVar-compatible mutex implementation.
//!
//! This provides mutex types specifically designed to work with condition variables.
//! These mutexes expose the necessary API for guards to be used with wait operations.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU8, Ordering};

use crate::primitives::{LockResult, PoisonError, SpinWait, TryLockError, TryLockResult};

// State encoding:
// Bit 0: LOCKED (1 = locked, 0 = unlocked)
// Bit 1: POISONED (1 = poisoned, 0 = clean)
const UNLOCKED: u8 = 0b00;
const LOCKED: u8 = 0b01;
const POISONED: u8 = 0b10;

/// A mutex specifically designed for use with condition variables.
///
/// This is similar to `SpinMutex` but exposes additional API needed for
/// condition variable operations.
///
/// # Examples
///
/// ```no_run
/// use foundation_nostd::primitives::{CondVar, CondVarMutex};
///
/// let mutex = CondVarMutex::new(false);
/// let condvar = CondVar::new();
///
/// // Wait for condition
/// let mut ready = mutex.lock().unwrap();
/// while !*ready {
///     ready = condvar.wait(ready).unwrap();
/// }
/// ```
pub struct CondVarMutex<T: ?Sized> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for CondVarMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for CondVarMutex<T> {}

/// RAII guard for `CondVarMutex`.
///
/// This guard exposes the parent mutex reference needed for condition variable operations.
pub struct CondVarMutexGuard<'a, T: ?Sized + 'a> {
    pub(crate) mutex: &'a CondVarMutex<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for CondVarMutexGuard<'_, T> {}

impl<T> CondVarMutex<T> {
    /// Creates a new unlocked mutex.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::CondVarMutex;
    ///
    /// let mutex = CondVarMutex::new(42);
    /// ```
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

impl<T: ?Sized> CondVarMutex<T> {
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
    pub fn lock(&self) -> LockResult<CondVarMutexGuard<'_, T>> {
        // Fast path: try to acquire immediately
        let state = self.state.load(Ordering::Relaxed);
        if state & LOCKED == 0
            && self
                .state
                .compare_exchange(state, state | LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            let guard = CondVarMutexGuard { mutex: self };
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
    fn lock_slow(&self) -> LockResult<CondVarMutexGuard<'_, T>> {
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
                let guard = CondVarMutexGuard { mutex: self };
                return if state & POISONED != 0 {
                    Err(PoisonError::new(guard))
                } else {
                    Ok(guard)
                };
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// # Errors
    ///
    /// Returns `Err(TryLockError::WouldBlock)` if the lock is already held,
    /// or `Err(TryLockError::Poisoned)` if the mutex was poisoned.
    #[inline]
    pub fn try_lock(&self) -> TryLockResult<CondVarMutexGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);
        if state & LOCKED == 0
            && self
                .state
                .compare_exchange(state, state | LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            let guard = CondVarMutexGuard { mutex: self };
            if state & POISONED != 0 {
                Err(TryLockError::Poisoned(PoisonError::new(guard)))
            } else {
                Ok(guard)
            }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }

    /// Unlocks the mutex.
    ///
    /// # Safety
    ///
    /// This must only be called by the thread that currently holds the lock.
    #[inline]
    pub(crate) unsafe fn unlock(&self) {
        self.state.fetch_and(!LOCKED, Ordering::Release);
    }

    /// Marks the mutex as poisoned.
    #[inline]
    #[allow(dead_code)] // Used by Drop impl when panicking
    fn poison(&self) {
        self.state.fetch_or(POISONED, Ordering::Relaxed);
    }
}

impl<'a, T: ?Sized> CondVarMutexGuard<'a, T> {
    /// Returns a reference to the parent mutex.
    ///
    /// This is needed for condition variable wait operations.
    #[inline]
    #[must_use]
    pub fn mutex(&self) -> &'a CondVarMutex<T> {
        self.mutex
    }
}

impl<T: ?Sized> Deref for CondVarMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for CondVarMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for CondVarMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // If panicking, poison the mutex
        #[cfg(feature = "std")]
        if std::thread::panicking() {
            self.mutex.poison();
        }

        // Unlock the mutex
        unsafe {
            self.mutex.unlock();
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for CondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for CondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for CondVarMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("CondVarMutex");
        match self.try_lock() {
            Ok(guard) => d.field("data", &&*guard),
            Err(TryLockError::Poisoned(ref e)) => d.field("data", &&**e.get_ref()),
            Err(TryLockError::WouldBlock) => d.field("data", &format_args!("<locked>")),
        };
        d.field("poisoned", &self.is_poisoned());
        d.finish_non_exhaustive()
    }
}

impl<T: Default> Default for CondVarMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for CondVarMutex<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

/// A raw mutex for use with `CondVarNonPoisoning`.
///
/// This is a simplified mutex without poisoning support.
pub struct RawCondVarMutex<T: ?Sized> {
    locked: core::sync::atomic::AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RawCondVarMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for RawCondVarMutex<T> {}

/// RAII guard for `RawCondVarMutex`.
pub struct RawCondVarMutexGuard<'a, T: ?Sized + 'a> {
    pub(crate) mutex: &'a RawCondVarMutex<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for RawCondVarMutexGuard<'_, T> {}

impl<T> RawCondVarMutex<T> {
    /// Creates a new unlocked mutex.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            locked: core::sync::atomic::AtomicBool::new(false),
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

impl<T: ?Sized> RawCondVarMutex<T> {
    /// Acquires the lock, spinning until it becomes available.
    #[inline]
    pub fn lock(&self) -> RawCondVarMutexGuard<'_, T> {
        // Fast path
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return RawCondVarMutexGuard { mutex: self };
        }

        // Slow path
        self.lock_slow()
    }

    #[cold]
    fn lock_slow(&self) -> RawCondVarMutexGuard<'_, T> {
        let mut spin_wait = SpinWait::new();
        loop {
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return RawCondVarMutexGuard { mutex: self };
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire the lock without blocking.
    #[inline]
    pub fn try_lock(&self) -> Option<RawCondVarMutexGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RawCondVarMutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Unlocks the mutex.
    ///
    /// # Safety
    ///
    /// This must only be called by the thread that currently holds the lock.
    #[inline]
    pub(crate) unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl<'a, T: ?Sized> RawCondVarMutexGuard<'a, T> {
    /// Returns a reference to the parent mutex.
    #[inline]
    #[must_use]
    pub fn mutex(&self) -> &'a RawCondVarMutex<T> {
        self.mutex
    }
}

impl<T: ?Sized> Deref for RawCondVarMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RawCondVarMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for RawCondVarMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.mutex.unlock();
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawCondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RawCondVarMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawCondVarMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RawCondVarMutex");
        match self.try_lock() {
            Some(guard) => d.field("data", &&*guard),
            None => d.field("data", &format_args!("<locked>")),
        };
        d.finish_non_exhaustive()
    }
}

impl<T: Default> Default for RawCondVarMutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for RawCondVarMutex<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Validate CondVarMutex can be created in const context
    /// WHAT: CondVarMutex::new() should work in const fn
    #[test]
    fn test_condvar_mutex_const_new() {
        const _MUTEX: CondVarMutex<i32> = CondVarMutex::new(42);
    }

    /// WHY: Validate RawCondVarMutex can be created in const context
    /// WHAT: RawCondVarMutex::new() should work in const fn
    #[test]
    fn test_raw_condvar_mutex_const_new() {
        const _MUTEX: RawCondVarMutex<i32> = RawCondVarMutex::new(42);
    }

    /// WHY: Validate basic lock/unlock operations
    /// WHAT: Should be able to lock, modify, and unlock
    #[test]
    fn test_condvar_mutex_basic_lock() {
        let mutex = CondVarMutex::new(0);
        {
            let mut guard = mutex.lock().unwrap();
            *guard = 42;
        }
        assert_eq!(*mutex.lock().unwrap(), 42);
    }

    /// WHY: Validate guard provides mutex access
    /// WHAT: Guard's `mutex()` method should return parent mutex
    #[test]
    fn test_condvar_mutex_guard_parent_access() {
        let mutex = CondVarMutex::new(0);
        let guard = mutex.lock().unwrap();
        let parent = guard.mutex();
        assert!(core::ptr::eq(parent, &raw const mutex));
    }
}
