//! No-op synchronization primitives for single-threaded WASM environments.
//!
//! These primitives provide `std::sync` compatibility for WASM targets
//! without atomics support (single-threaded). They use simple `UnsafeCell`
//! wrappers without any atomic operations.
//!
//! # Safety
//!
//! These types are ONLY safe in single-threaded environments. Using them
//! in multi-threaded contexts will cause undefined behavior.
//!
//! # Examples
//!
//! ```ignore
//! # #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
//! use foundation_nostd::primitives::noop::{NoopMutex, NoopRwLock};
//!
//! # #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
//! let mutex = NoopMutex::new(42);
//! # #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
//! let mut guard = mutex.lock();
//! # #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
//! *guard = 100;
//! ```ignore

use core::cell::{Cell, UnsafeCell};
use core::fmt;
use core::ops::{Deref, DerefMut};

use crate::primitives::{TryLockError, TryLockResult};

/// A no-op mutex for single-threaded WASM environments.
///
/// This provides a `std::sync::Mutex`-compatible API without any actual
/// locking mechanism, suitable for single-threaded WASM without atomics.
///
/// # Safety
///
/// Only safe in single-threaded contexts. Multi-threaded use causes UB.
pub struct NoopMutex<T: ?Sized> {
    locked: Cell<bool>,
    data: UnsafeCell<T>,
}

/// RAII guard for `NoopMutex`.
pub struct NoopMutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a NoopMutex<T>,
}

impl<T> NoopMutex<T> {
    /// Creates a new unlocked mutex.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopMutex;
    ///
    /// let mutex = NoopMutex::new(42);
    /// ```ignore
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            locked: Cell::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the mutex and returns the inner value.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopMutex;
    ///
    /// let mutex = NoopMutex::new(42);
    /// let value = mutex.into_inner();
    /// assert_eq!(value, 42);
    /// ```ignore
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopMutex;
    ///
    /// let mut mutex = NoopMutex::new(0);
    /// *mutex.get_mut() = 10;
    /// assert_eq!(*mutex.lock(), 10);
    /// ```ignore
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: ?Sized> NoopMutex<T> {
    /// Acquires the lock.
    ///
    /// In single-threaded contexts, this always succeeds immediately.
    ///
    /// # Panics
    ///
    /// Panics if the lock is already held (recursive lock attempt).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopMutex;
    ///
    /// let mutex = NoopMutex::new(0);
    /// let mut guard = mutex.lock();
    /// *guard += 1;
    /// ```ignore
    #[inline]
    pub fn lock(&self) -> NoopMutexGuard<'_, T> {
        assert!(
            !self.locked.get(),
            "NoopMutex: recursive lock attempt in single-threaded context"
        );
        self.locked.set(true);
        NoopMutexGuard { mutex: self }
    }

    /// Attempts to acquire the lock.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopMutex;
    ///
    /// let mutex = NoopMutex::new(42);
    ///
    /// let _ = mutex.try_lock();
    ///     assert_eq!(*guard, 42);
    /// ```ignore
    #[inline]
    pub fn try_lock(&self) -> TryLockResult<NoopMutexGuard<'_, T>> {
        if self.locked.get() {
            Err(TryLockError::WouldBlock)
        } else {
            self.locked.set(true);
            Ok(NoopMutexGuard { mutex: self })
        }
    }

    /// Checks if the mutex is currently locked.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopMutex;
    ///
    /// let mutex = NoopMutex::new(0);
    /// assert!(!mutex.is_locked());
    /// let _guard = mutex.lock();
    /// assert!(mutex.is_locked());
    /// ```ignore
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.locked.get()
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for NoopMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f.debug_struct("NoopMutex").field("data", &&*guard).finish(),
            Err(_) => f
                .debug_struct("NoopMutex")
                .field("data", &"<locked>")
                .finish(),
        }
    }
}

impl<T: Default> Default for NoopMutex<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for NoopMutex<T> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> Deref for NoopMutexGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for NoopMutexGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for NoopMutexGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.locked.set(false);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for NoopMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for NoopMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// A no-op read-write lock for single-threaded WASM environments.
///
/// This provides a `std::sync::RwLock`-compatible API without any actual
/// locking mechanism, suitable for single-threaded WASM without atomics.
///
/// # Safety
///
/// Only safe in single-threaded contexts. Multi-threaded use causes UB.
pub struct NoopRwLock<T: ?Sized> {
    locked: Cell<LockState>,
    data: UnsafeCell<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockState {
    Unlocked,
    Reading(usize), // Number of readers
    Writing,
}

/// RAII read guard for `NoopRwLock`.
pub struct NoopReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a NoopRwLock<T>,
}

/// RAII write guard for `NoopRwLock`.
pub struct NoopWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a NoopRwLock<T>,
}

impl<T> NoopRwLock<T> {
    /// Creates a new unlocked RwLock.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let lock = NoopRwLock::new(42);
    /// ```ignore
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            locked: Cell::new(LockState::Unlocked),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the lock and returns the inner value.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let lock = NoopRwLock::new(42);
    /// let value = lock.into_inner();
    /// assert_eq!(value, 42);
    /// ```ignore
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let mut lock = NoopRwLock::new(0);
    /// *lock.get_mut() = 10;
    /// assert_eq!(*lock.read(), 10);
    /// ```ignore
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
}

impl<T: ?Sized> NoopRwLock<T> {
    /// Acquires a read lock.
    ///
    /// Multiple readers can hold the lock simultaneously in single-threaded contexts.
    ///
    /// # Panics
    ///
    /// Panics if a write lock is currently held.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let lock = NoopRwLock::new(42);
    /// let r1 = lock.read();
    /// let r2 = lock.read();  // Multiple readers OK
    /// assert_eq!(*r1, 42);
    /// ```ignore
    #[inline]
    pub fn read(&self) -> NoopReadGuard<'_, T> {
        match self.locked.get() {
            LockState::Unlocked => {
                self.locked.set(LockState::Reading(1));
            }
            LockState::Reading(n) => {
                self.locked.set(LockState::Reading(n + 1));
            }
            LockState::Writing => {
                panic!("NoopRwLock: cannot read while write lock is held");
            }
        }
        NoopReadGuard { lock: self }
    }

    /// Attempts to acquire a read lock.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let lock = NoopRwLock::new(42);
    ///
    /// let _ = lock.try_read();
    ///     assert_eq!(*guard, 42);
    /// ```ignore
    #[inline]
    pub fn try_read(&self) -> TryLockResult<NoopReadGuard<'_, T>> {
        match self.locked.get() {
            LockState::Unlocked => {
                self.locked.set(LockState::Reading(1));
                Ok(NoopReadGuard { lock: self })
            }
            LockState::Reading(n) => {
                self.locked.set(LockState::Reading(n + 1));
                Ok(NoopReadGuard { lock: self })
            }
            LockState::Writing => Err(TryLockError::WouldBlock),
        }
    }

    /// Acquires a write lock.
    ///
    /// Only one writer can hold the lock at a time.
    ///
    /// # Panics
    ///
    /// Panics if any lock (read or write) is currently held.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let lock = NoopRwLock::new(0);
    /// let mut w = lock.write();
    /// *w += 1;
    /// ```ignore
    #[inline]
    pub fn write(&self) -> NoopWriteGuard<'_, T> {
        match self.locked.get() {
            LockState::Unlocked => {
                self.locked.set(LockState::Writing);
                NoopWriteGuard { lock: self }
            }
            _ => {
                panic!("NoopRwLock: cannot write while lock is held");
            }
        }
    }

    /// Attempts to acquire a write lock.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopRwLock;
    ///
    /// let lock = NoopRwLock::new(42);
    ///
    /// let _ = lock.try_write();
    ///     *guard = 100;
    /// ```ignore
    #[inline]
    pub fn try_write(&self) -> TryLockResult<NoopWriteGuard<'_, T>> {
        match self.locked.get() {
            LockState::Unlocked => {
                self.locked.set(LockState::Writing);
                Ok(NoopWriteGuard { lock: self })
            }
            _ => Err(TryLockError::WouldBlock),
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for NoopRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_read() {
            Ok(guard) => f
                .debug_struct("NoopRwLock")
                .field("data", &&*guard)
                .finish(),
            Err(_) => f
                .debug_struct("NoopRwLock")
                .field("data", &"<locked>")
                .finish(),
        }
    }
}

impl<T: Default> Default for NoopRwLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for NoopRwLock<T> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> Deref for NoopReadGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for NoopReadGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        match self.lock.locked.get() {
            LockState::Reading(1) => {
                self.lock.locked.set(LockState::Unlocked);
            }
            LockState::Reading(n) => {
                self.lock.locked.set(LockState::Reading(n - 1));
            }
            _ => unreachable!("NoopReadGuard dropped with invalid lock state"),
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for NoopReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for NoopReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for NoopWriteGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for NoopWriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for NoopWriteGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.locked.set(LockState::Unlocked);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for NoopWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for NoopWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// A no-op once cell for single-threaded WASM environments.
///
/// This provides a `std::sync::Once`-compatible API without any actual
/// synchronization mechanism, suitable for single-threaded WASM without atomics.
///
/// # Safety
///
/// Only safe in single-threaded contexts. Multi-threaded use causes UB.
pub struct NoopOnce {
    called: Cell<bool>,
}

impl NoopOnce {
    /// Creates a new `NoopOnce` value.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopOnce;
    ///
    /// const INIT: NoopOnce = NoopOnce::new();
    /// ```ignore
    #[inline]
    pub const fn new() -> Self {
        Self {
            called: Cell::new(false),
        }
    }

    /// Performs an initialization routine once and only once.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopOnce;
    ///
    /// static INIT: NoopOnce = NoopOnce::new();
    ///
    /// INIT.call_once(|| {
    ///     // This will only be called once
    /// });
    /// ```ignore
    #[inline]
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        if !self.called.get() {
            self.called.set(true);
            f();
        }
    }

    /// Returns whether the initialization has been performed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_nostd::primitives::noop::NoopOnce;
    ///
    /// let once = NoopOnce::new();
    /// assert!(!once.is_completed());
    /// once.call_once(|| {});
    /// assert!(once.is_completed());
    /// ```ignore
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.called.get()
    }
}

impl Default for NoopOnce {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NoopOnce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoopOnce")
            .field("called", &self.called.get())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    // NoopMutex tests
    #[test]
    fn test_mutex_new_and_into_inner() {
        let mutex = NoopMutex::new(42);
        assert_eq!(mutex.into_inner(), 42);
    }

    #[test]
    fn test_mutex_lock() {
        let mutex = NoopMutex::new(0);
        {
            let mut guard = mutex.lock();
            *guard += 1;
            assert_eq!(*guard, 1);
        }
        let guard = mutex.lock();
        assert_eq!(*guard, 1);
    }

    #[test]
    fn test_mutex_try_lock_success() {
        let mutex = NoopMutex::new(42);
        let guard = mutex.try_lock();
        assert!(guard.is_ok());
        assert_eq!(*guard.unwrap(), 42);
    }

    #[test]
    fn test_mutex_try_lock_would_block() {
        let mutex = NoopMutex::new(42);
        let _guard1 = mutex.lock();
        let result = mutex.try_lock();
        assert!(matches!(result, Err(TryLockError::WouldBlock)));
    }

    #[test]
    fn test_mutex_is_locked() {
        let mutex = NoopMutex::new(0);
        assert!(!mutex.is_locked());
        let _guard = mutex.lock();
        assert!(mutex.is_locked());
    }

    #[test]
    fn test_mutex_get_mut() {
        let mut mutex = NoopMutex::new(0);
        *mutex.get_mut() = 42;
        assert_eq!(*mutex.lock(), 42);
    }

    // NoopRwLock tests
    #[test]
    fn test_rwlock_new_and_into_inner() {
        let lock = NoopRwLock::new(42);
        assert_eq!(lock.into_inner(), 42);
    }

    #[test]
    fn test_rwlock_read() {
        let lock = NoopRwLock::new(42);
        let r = lock.read();
        assert_eq!(*r, 42);
    }

    #[test]
    fn test_rwlock_multiple_readers() {
        let lock = NoopRwLock::new(42);
        let r1 = lock.read();
        let r2 = lock.read();
        let r3 = lock.read();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
        assert_eq!(*r3, 42);
    }

    #[test]
    fn test_rwlock_write() {
        let lock = NoopRwLock::new(0);
        {
            let mut w = lock.write();
            *w += 1;
            assert_eq!(*w, 1);
        }
        let r = lock.read();
        assert_eq!(*r, 1);
    }

    #[test]
    fn test_rwlock_try_read_success() {
        let lock = NoopRwLock::new(42);
        let r = lock.try_read();
        assert!(r.is_ok());
        assert_eq!(*r.unwrap(), 42);
    }

    #[test]
    fn test_rwlock_try_write_success() {
        let lock = NoopRwLock::new(42);
        let w = lock.try_write();
        assert!(w.is_ok());
    }

    #[test]
    fn test_rwlock_get_mut() {
        let mut lock = NoopRwLock::new(0);
        *lock.get_mut() = 42;
        assert_eq!(*lock.read(), 42);
    }

    // NoopOnce tests
    #[test]
    fn test_once_new() {
        let once = NoopOnce::new();
        assert!(!once.is_completed());
    }

    #[test]
    fn test_once_call_once() {
        let once = NoopOnce::new();
        let mut called = 0;

        once.call_once(|| {
            called += 1;
        });
        assert_eq!(called, 1);
        assert!(once.is_completed());

        once.call_once(|| {
            called += 1;
        });
        assert_eq!(called, 1); // Should not be called again
    }

    #[test]
    fn test_once_debug() {
        let once = NoopOnce::new();
        let debug_str = format!("{:?}", once);
        assert!(debug_str.contains("NoopOnce"));
    }
}
