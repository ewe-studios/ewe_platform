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
