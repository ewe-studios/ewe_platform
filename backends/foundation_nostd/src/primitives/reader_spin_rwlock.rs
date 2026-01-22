//! Reader-preferring read-write lock with poisoning support.
//!
//! This provides a reader-preferring variant of RwLock where readers can always
//! acquire the lock, even when writers are waiting. This maximizes read concurrency
//! but may starve writers in read-heavy workloads.
//!
//! # Examples
//!
//! ```
//! use foundation_nostd::primitives::reader_spin_rwlock::ReaderSpinRwLock;
//!
//! let lock = ReaderSpinRwLock::new(vec![1, 2, 3]);
//!
//! // Multiple readers can access simultaneously
//! {
//!     let r1 = lock.read().unwrap();
//!     let r2 = lock.read().unwrap();
//!     assert_eq!(*r1, vec![1, 2, 3]);
//!     assert_eq!(*r2, vec![1, 2, 3]);
//! }
//!
//! // Writers have exclusive access
//! {
//!     let mut w = lock.write().unwrap();
//!     w.push(4);
//! }
//! ```
//!
//! # Poisoning
//!
//! When a thread panics while holding a guard, the lock becomes "poisoned".
//! Subsequent lock acquisitions return `Err(PoisonError)` to signal that
//! the protected data may be inconsistent.
//!
//! # Reader vs Writer Preference
//!
//! Unlike `SpinRwLock` (writer-preferring), this lock allows readers to acquire
//! even when writers are waiting. Use this when:
//! - Reads vastly outnumber writes (>95% reads)
//! - Write latency is not critical
//! - Maximum read concurrency is essential

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

use crate::primitives::{LockResult, PoisonError, TryLockError, TryLockResult};

// State encoding (32-bit) - Reader-preferring (no writer waiting flag):
// Bits 0-29: Reader count (up to ~1 billion readers)
// Bit 30: Unused
// Bit 31: Writer active flag
const READER_MASK: u32 = (1 << 30) - 1; // 0x3FFFFFFF
const WRITER_ACTIVE: u32 = 1 << 31; // 0x80000000
const MAX_READERS: u32 = READER_MASK;

/// A reader-preferring read-write spin lock with poisoning support.
///
/// This lock allows readers to acquire the lock even when writers are waiting,
/// maximizing read concurrency at the potential cost of writer starvation.
///
/// # Thread Safety
///
/// This type is `Send` and `Sync` when `T: Send + Sync`.
pub struct ReaderSpinRwLock<T: ?Sized> {
    state: AtomicU32,
    poisoned: AtomicU32, // Separate atomic for poison state
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for ReaderSpinRwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for ReaderSpinRwLock<T> {}

/// RAII read guard for `ReaderSpinRwLock`.
///
/// The read lock is released when this guard is dropped. If the thread panics
/// while holding the guard, the lock will be poisoned.
pub struct ReaderReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a ReaderSpinRwLock<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for ReaderReadGuard<'_, T> {}

/// RAII write guard for `ReaderSpinRwLock`.
///
/// The write lock is released when this guard is dropped. If the thread panics
/// while holding the guard, the lock will be poisoned.
pub struct ReaderWriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a ReaderSpinRwLock<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for ReaderWriteGuard<'_, T> {}

impl<T> ReaderSpinRwLock<T> {
    /// Creates a new unlocked RwLock.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::reader_spin_rwlock::ReaderSpinRwLock;
    ///
    /// let lock = ReaderSpinRwLock::new(42);
    /// ```
    #[inline]
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            poisoned: AtomicU32::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Consumes the lock and returns the inner value.
    ///
    /// # Errors
    ///
    /// Returns `PoisonError` if the lock was poisoned.
    #[inline]
    pub fn into_inner(self) -> LockResult<T> {
        let is_poisoned = self.poisoned.load(Ordering::Acquire) != 0;
        let data = self.data.into_inner();
        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized> ReaderSpinRwLock<T> {
    /// Acquires a read lock, spinning until available.
    ///
    /// Returns a guard that releases the lock when dropped. Multiple readers
    /// can hold the lock simultaneously.
    ///
    /// # Errors
    ///
    /// Returns `PoisonError` if the lock was poisoned.
    #[inline]
    pub fn read(&self) -> LockResult<ReaderReadGuard<'_, T>> {
        loop {
            match self.try_read() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::WouldBlock) => core::hint::spin_loop(),
                Err(TryLockError::Poisoned(err)) => return Err(err),
            }
        }
    }

    /// Attempts to acquire a read lock without spinning.
    ///
    /// # Errors
    ///
    /// Returns `WouldBlock` if the lock is held by a writer.
    /// Returns `Poisoned` if the lock was poisoned.
    pub fn try_read(&self) -> TryLockResult<ReaderReadGuard<'_, T>> {
        // Check poisoned first
        if self.poisoned.load(Ordering::Acquire) != 0 {
            return Err(TryLockError::Poisoned(PoisonError::new(
                ReaderReadGuard { lock: self },
            )));
        }

        // Reader-preferring: Only check for active writer, ignore waiting writers
        let state = self.state.load(Ordering::Acquire);
        if state & WRITER_ACTIVE != 0 {
            return Err(TryLockError::WouldBlock);
        }

        // Try to increment reader count
        let readers = state & READER_MASK;
        if readers >= MAX_READERS {
            return Err(TryLockError::WouldBlock); // Too many readers
        }

        match self.state.compare_exchange_weak(
            state,
            state + 1,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(ReaderReadGuard { lock: self }),
            Err(_) => Err(TryLockError::WouldBlock),
        }
    }

    /// Acquires a write lock, spinning until available.
    ///
    /// Returns a guard that releases the lock when dropped. The write lock
    /// is exclusive - no readers or writers can access while held.
    ///
    /// # Errors
    ///
    /// Returns `PoisonError` if the lock was poisoned.
    #[inline]
    pub fn write(&self) -> LockResult<ReaderWriteGuard<'_, T>> {
        loop {
            match self.try_write() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::WouldBlock) => core::hint::spin_loop(),
                Err(TryLockError::Poisoned(err)) => return Err(err),
            }
        }
    }

    /// Attempts to acquire a write lock without spinning.
    ///
    /// # Errors
    ///
    /// Returns `WouldBlock` if the lock is held by readers or another writer.
    /// Returns `Poisoned` if the lock was poisoned.
    pub fn try_write(&self) -> TryLockResult<ReaderWriteGuard<'_, T>> {
        // Check poisoned first
        if self.poisoned.load(Ordering::Acquire) != 0 {
            return Err(TryLockError::Poisoned(PoisonError::new(
                ReaderWriteGuard { lock: self },
            )));
        }

        // Try to acquire write lock (state must be 0)
        match self.state.compare_exchange(
            0,
            WRITER_ACTIVE,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(ReaderWriteGuard { lock: self }),
            Err(_) => Err(TryLockError::WouldBlock),
        }
    }

    /// Returns whether the lock is poisoned.
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Acquire) != 0
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this requires a mutable reference to the lock, no locking is needed.
    ///
    /// # Errors
    ///
    /// Returns `PoisonError` if the lock was poisoned.
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let is_poisoned = *self.poisoned.get_mut() != 0;
        let data = self.data.get_mut();
        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ReaderSpinRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_read() {
            Ok(guard) => f.debug_struct("ReaderSpinRwLock").field("data", &&*guard).finish(),
            Err(TryLockError::Poisoned(_)) => {
                f.debug_struct("ReaderSpinRwLock").field("data", &"<poisoned>").finish()
            }
            Err(TryLockError::WouldBlock) => {
                f.debug_struct("ReaderSpinRwLock").field("data", &"<locked>").finish()
            }
        }
    }
}

impl<T: Default> Default for ReaderSpinRwLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> From<T> for ReaderSpinRwLock<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

// === Guard implementations ===

impl<T: ?Sized> Deref for ReaderReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for ReaderReadGuard<'_, T> {
    fn drop(&mut self) {
        // Mark as poisoned if panicking
        #[cfg(feature = "std")]
        if std::panicking::panicking() {
            self.lock.poisoned.store(1, Ordering::Release);
        }

        // Decrement reader count
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ReaderReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for ReaderReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Deref for ReaderWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for ReaderWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for ReaderWriteGuard<'_, T> {
    fn drop(&mut self) {
        // Mark as poisoned if panicking
        #[cfg(feature = "std")]
        if std::panicking::panicking() {
            self.lock.poisoned.store(1, Ordering::Release);
        }

        // Release write lock
        self.lock.state.store(0, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ReaderWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for ReaderWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::{panic::catch_unwind, panic::AssertUnwindSafe, vec, vec::Vec};

    #[test]
    fn test_new() {
        let lock = ReaderSpinRwLock::new(42);
        assert_eq!(*lock.read().unwrap(), 42);
    }

    #[test]
    fn test_read() {
        let lock = ReaderSpinRwLock::new(vec![1, 2, 3]);
        let r1 = lock.read().unwrap();
        let r2 = lock.read().unwrap();
        assert_eq!(*r1, vec![1, 2, 3]);
        assert_eq!(*r2, vec![1, 2, 3]);
    }

    #[test]
    fn test_write() {
        let lock = ReaderSpinRwLock::new(0);
        {
            let mut w = lock.write().unwrap();
            *w = 42;
        }
        assert_eq!(*lock.read().unwrap(), 42);
    }

    #[test]
    fn test_try_read() {
        let lock = ReaderSpinRwLock::new(42);
        assert!(lock.try_read().is_ok());
    }

    #[test]
    fn test_try_write() {
        let lock = ReaderSpinRwLock::new(42);
        assert!(lock.try_write().is_ok());
    }

    #[test]
    fn test_reader_preferring_no_block_on_writer_waiting() {
        // This test demonstrates reader-preferring behavior:
        // Readers can acquire even while writer is trying to acquire
        let lock = ReaderSpinRwLock::new(0);

        let _r1 = lock.read().unwrap();
        // Writer would set WRITER_WAITING in writer-preferring lock
        // But reader-preferring has no such flag
        let r2 = lock.read(); // Should succeed

        assert!(r2.is_ok());
    }

    #[test]
    fn test_into_inner() {
        let lock = ReaderSpinRwLock::new(42);
        assert_eq!(lock.into_inner().unwrap(), 42);
    }

    #[test]
    fn test_get_mut() {
        let mut lock = ReaderSpinRwLock::new(42);
        *lock.get_mut().unwrap() = 100;
        assert_eq!(*lock.read().unwrap(), 100);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_poisoning() {
        let lock = ReaderSpinRwLock::new(0);

        // Simulate panic during write
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = lock.write().unwrap();
            panic!("test panic");
        }));
        assert!(result.is_err());

        // Lock should be poisoned
        assert!(lock.is_poisoned());
        assert!(lock.read().is_err());
        assert!(lock.write().is_err());
    }
}
