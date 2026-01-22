//! Raw read-write spin lock without poisoning support.
//!
//! This provides a writer-preferring read-write lock for `no_std` environments.
//! Multiple readers can access the data simultaneously, but writers have
//! exclusive access.

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

use crate::primitives::spin_wait::SpinWait;

// State encoding (32-bit):
// Bits 0-29: Reader count (up to ~1 billion readers)
// Bit 30: Writer waiting flag
// Bit 31: Writer active flag
const READER_MASK: u32 = (1 << 30) - 1;
const WRITER_WAITING: u32 = 1 << 30;
const WRITER_ACTIVE: u32 = 1 << 31;
const MAX_READERS: u32 = READER_MASK;

/// A writer-preferring read-write spin lock without poisoning.
pub struct RawSpinRwLock<T: ?Sized> {
    state: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RawSpinRwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RawSpinRwLock<T> {}

/// RAII read guard for `RawSpinRwLock`.
pub struct RawReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RawSpinRwLock<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for RawReadGuard<'_, T> {}

/// RAII write guard for `RawSpinRwLock`.
pub struct RawWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RawSpinRwLock<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for RawWriteGuard<'_, T> {}

impl<T> RawSpinRwLock<T> {
    /// Creates a new unlocked RwLock.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the lock and returns the inner value.
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

impl<T: ?Sized> RawSpinRwLock<T> {
    /// Acquires a read lock, spinning until available.
    #[inline]
    pub fn read(&self) -> RawReadGuard<'_, T> {
        // Fast path: no writers waiting/active, increment reader count
        let state = self.state.load(Ordering::Relaxed);
        if state & (WRITER_ACTIVE | WRITER_WAITING) == 0
            && state < MAX_READERS
            && self
                .state
                .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            return RawReadGuard { lock: self };
        }

        // Slow path
        self.read_slow()
    }

    #[cold]
    fn read_slow(&self) -> RawReadGuard<'_, T> {
        let mut spin_wait = SpinWait::new();
        loop {
            let state = self.state.load(Ordering::Relaxed);
            // Wait if writer is waiting/active
            if state & (WRITER_ACTIVE | WRITER_WAITING) == 0 && state < MAX_READERS {
                if self
                    .state
                    .compare_exchange_weak(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return RawReadGuard { lock: self };
                }
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire a read lock without blocking.
    #[inline]
    pub fn try_read(&self) -> Option<RawReadGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);
        if state & (WRITER_ACTIVE | WRITER_WAITING) == 0
            && state < MAX_READERS
            && self
                .state
                .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        {
            Some(RawReadGuard { lock: self })
        } else {
            None
        }
    }

    /// Acquires a write lock, spinning until available.
    #[inline]
    pub fn write(&self) -> RawWriteGuard<'_, T> {
        // Fast path: no readers or writers
        if self
            .state
            .compare_exchange(0, WRITER_ACTIVE, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return RawWriteGuard { lock: self };
        }

        // Slow path
        self.write_slow()
    }

    #[cold]
    fn write_slow(&self) -> RawWriteGuard<'_, T> {
        let mut spin_wait = SpinWait::new();

        // Set writer waiting flag
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & WRITER_WAITING == 0 {
                if self
                    .state
                    .compare_exchange_weak(
                        state,
                        state | WRITER_WAITING,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    break;
                }
            }
            spin_wait.spin();
        }

        // Wait for all readers to finish
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state & READER_MASK == 0 && state & WRITER_ACTIVE == 0 {
                // Clear waiting flag and set active flag
                if self
                    .state
                    .compare_exchange(state, WRITER_ACTIVE, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return RawWriteGuard { lock: self };
                }
            }
            spin_wait.spin();
        }
    }

    /// Attempts to acquire a write lock without blocking.
    #[inline]
    pub fn try_write(&self) -> Option<RawWriteGuard<'_, T>> {
        if self
            .state
            .compare_exchange(0, WRITER_ACTIVE, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RawWriteGuard { lock: self })
        } else {
            None
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawSpinRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_read() {
            Some(guard) => f
                .debug_struct("RawSpinRwLock")
                .field("data", &&*guard)
                .finish(),
            None => f
                .debug_struct("RawSpinRwLock")
                .field("data", &"<locked>")
                .finish(),
        }
    }
}

impl<T: Default> Default for RawSpinRwLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for RawSpinRwLock<T> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> Deref for RawReadGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for RawReadGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for RawWriteGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for RawWriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for RawWriteGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.state.store(0, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RawWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Validates basic construction and into_inner
    /// WHAT: Creating lock and extracting value should work
    #[test]
    fn test_new_and_into_inner() {
        let lock = RawSpinRwLock::new(42);
        assert_eq!(lock.into_inner(), 42);
    }

    /// WHY: Validates read lock acquisition
    /// WHAT: Read lock should be acquirable
    #[test]
    fn test_read() {
        let lock = RawSpinRwLock::new(42);
        let guard = lock.read();
        assert_eq!(*guard, 42);
    }

    /// WHY: Validates write lock acquisition
    /// WHAT: Write lock should be acquirable and allow mutation
    #[test]
    fn test_write() {
        let lock = RawSpinRwLock::new(0);
        {
            let mut guard = lock.write();
            *guard = 42;
        }
        assert_eq!(*lock.read(), 42);
    }

    /// WHY: Validates multiple simultaneous readers
    /// WHAT: Multiple read guards should coexist
    #[test]
    fn test_multiple_readers() {
        let lock = RawSpinRwLock::new(42);
        let r1 = lock.read();
        let r2 = lock.read();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
    }

    /// WHY: Validates try_read when lock is free
    /// WHAT: try_read should succeed when no writers
    #[test]
    fn test_try_read_success() {
        let lock = RawSpinRwLock::new(42);
        let guard = lock.try_read();
        assert!(guard.is_some());
    }

    /// WHY: Validates try_write when lock is free
    /// WHAT: try_write should succeed when unlocked
    #[test]
    fn test_try_write_success() {
        let lock = RawSpinRwLock::new(42);
        let guard = lock.try_write();
        assert!(guard.is_some());
    }

    /// WHY: Validates Send trait bounds
    /// WHAT: RwLock should be Send when T is Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RawSpinRwLock<i32>>();
    }

    /// WHY: Validates Sync trait bounds
    /// WHAT: RwLock should be Sync when T is Send + Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RawSpinRwLock<i32>>();
    }
}
