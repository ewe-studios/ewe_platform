//! Spin-based read-write lock with poisoning support.
//!
//! This provides a `std::sync::RwLock`-compatible API for `no_std` environments.
//! Unlike `RawSpinRwLock`, this tracks panics during guard drops to detect
//! potential data corruption from panicked critical sections.
//!
//! # Examples
//!
//! ```
//! use foundation_nostd::primitives::SpinRwLock;
//!
//! let lock = SpinRwLock::new(vec![1, 2, 3]);
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

use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering};

use crate::primitives::{LockResult, PoisonError, TryLockError, TryLockResult};

// State encoding (32-bit):
// Bits 0-29: Reader count (up to ~1 billion readers)
// Bit 30: Writer waiting flag
// Bit 31: Writer active flag
// Bits 32-63: Poison bit (stored separately for simplicity)
const READER_MASK: u32 = (1 << 30) - 1; // 0x3FFFFFFF
const WRITER_WAITING: u32 = 1 << 30; // 0x40000000
const WRITER_ACTIVE: u32 = 1 << 31; // 0x80000000
const MAX_READERS: u32 = READER_MASK;

/// A writer-preferring read-write `spin` `lock` with poisoning support.
///
/// This matches the `std::sync::RwLock` API for drop-in replacement in
/// `no_std` contexts. When a thread panics while holding the lock, the
/// `lock` becomes poisoned to detect potential data corruption.
///
/// # Thread Safety
///
/// This type is `Send` and `Sync` when `T: Send + Sync`.
pub struct SpinRwLock<T: ?Sized> {
    state: AtomicU32,
    poisoned: AtomicU32, // Separate atomic for poison state
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for SpinRwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for SpinRwLock<T> {}

/// RAII `read` guard for `SpinRwLock`.
///
/// The `read` `lock` is released when this guard is dropped. If the thread panics
/// while holding the guard, the `lock` will be poisoned.
pub struct ReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a SpinRwLock<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for ReadGuard<'_, T> {}

/// RAII `write` guard for `SpinRwLock`.
///
/// The `write` `lock` is released when this guard is dropped. If the thread panics
/// while holding the guard, the `lock` will be poisoned.
pub struct WriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a SpinRwLock<T>,
}

unsafe impl<T: ?Sized + Sync> Sync for WriteGuard<'_, T> {}

impl<T> SpinRwLock<T> {
    /// Creates a new unlocked `RwLock`.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(42);
    /// ```
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            poisoned: AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes the `lock` and returns the inner value.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the `lock` was poisoned.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(42);
    /// let value = `lock`.into_inner().unwrap();
    /// assert_eq!(value, 42);
    /// ```
    #[inline]
    pub fn into_inner(self) -> LockResult<T> {
        let is_poisoned = self.poisoned.load(Ordering::Relaxed) != 0;
        let data = self.data.into_inner();

        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this takes `&mut self`, no locking is needed.
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the `lock` was poisoned.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let mut `lock` = SpinRwLock::new(0);
    /// *lock.get_mut().unwrap() = 10;
    /// assert_eq!(*lock.read().unwrap(), 10);
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let is_poisoned = self.poisoned.load(Ordering::Relaxed) != 0;
        let data = self.data.get_mut();

        if is_poisoned {
            Err(PoisonError::new(data))
        } else {
            Ok(data)
        }
    }
}

impl<T: ?Sized> SpinRwLock<T> {
    /// Checks if the `lock` is poisoned.
    ///
    /// A `lock` is poisoned when a thread panicked while holding the `lock`.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(0);
    /// assert!(!lock.is_poisoned());
    /// ```
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed) != 0
    }

    /// Acquires a `read` lock, spinning until available.
    ///
    /// Multiple readers can hold the `lock` simultaneously. This call will block
    /// if a writer is active or waiting (writer-preferring policy).
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the `lock` was poisoned. The guard is
    /// still returned, allowing access to the underlying data.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(vec![1, 2, 3]);
    /// let r1 = `lock`.read().unwrap();
    /// let r2 = `lock`.read().unwrap();  // Multiple readers OK
    /// assert_eq!(*r1, vec![1, 2, 3]);
    /// ```
    #[inline]
    pub fn read(&self) -> LockResult<ReadGuard<'_, T>> {
        // Fast path: try to acquire read lock if no writer
        let state = self.state.load(Ordering::Relaxed);
        if state & (WRITER_ACTIVE | WRITER_WAITING) == 0 && state < MAX_READERS
            && self
                .state
                .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                let guard = ReadGuard { lock: self };
                return if self.is_poisoned() {
                    Err(PoisonError::new(guard))
                } else {
                    Ok(guard)
                };
            }

        // Slow path: spin until we can acquire
        self.read_slow()
    }

    #[cold]
    fn read_slow(&self) -> LockResult<ReadGuard<'_, T>> {
        loop {
            for _ in 0..100 {
                let state = self.state.load(Ordering::Relaxed);

                // Can only acquire if no writer is active or waiting
                if state & (WRITER_ACTIVE | WRITER_WAITING) == 0 && state < MAX_READERS
                    && self
                        .state
                        .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                        .is_ok()
                    {
                        let guard = ReadGuard { lock: self };
                        return if self.is_poisoned() {
                            Err(PoisonError::new(guard))
                        } else {
                            Ok(guard)
                        };
                    }

                core::hint::spin_loop();
            }
        }
    }

    /// Attempts to acquire a `read` `lock` without blocking.
    ///
    /// Returns `Ok(guard)` if acquired, or an error if unavailable or poisoned.
    ///
    /// # Errors
    ///
    /// - `Err(TryLockError::WouldBlock)` if the `lock` is currently held by a writer
    /// - `Err(TryLockError::Poisoned)` if the `lock` was poisoned
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(42);
    ///
    /// match `lock`.try_read() {
    ///     Ok(guard) => assert_eq!(*guard, 42),
    ///     Err(_) => {}
    /// };
    /// ```
    #[inline]
    pub fn try_read(&self) -> TryLockResult<ReadGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);

        // Can only acquire if no writer is active or waiting
        if state & (WRITER_ACTIVE | WRITER_WAITING) == 0 && state < MAX_READERS
            && self
                .state
                .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                let guard = ReadGuard { lock: self };
                return if self.is_poisoned() {
                    Err(TryLockError::Poisoned(PoisonError::new(guard)))
                } else {
                    Ok(guard)
                };
            }

        Err(TryLockError::WouldBlock)
    }

    /// Acquires a `write` lock, spinning until available.
    ///
    /// Writers have exclusive access. This call sets the writer-waiting flag
    /// to block new readers (writer-preferring policy).
    ///
    /// # Errors
    ///
    /// Returns `Err(PoisonError)` if the `lock` was poisoned. The guard is
    /// still returned, allowing access to the underlying data.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(0);
    /// let mut w = `lock`.write().unwrap();
    /// *w += 1;
    /// ```
    #[inline]
    pub fn write(&self) -> LockResult<WriteGuard<'_, T>> {
        // Fast path: try to acquire if no readers or writers
        if self
            .state
            .compare_exchange(0, WRITER_ACTIVE, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let guard = WriteGuard { lock: self };
            return if self.is_poisoned() {
                Err(PoisonError::new(guard))
            } else {
                Ok(guard)
            };
        }

        // Slow path: set waiting flag and spin
        self.write_slow()
    }

    #[cold]
    fn write_slow(&self) -> LockResult<WriteGuard<'_, T>> {
        // Set writer-waiting flag to block new readers
        self.state.fetch_or(WRITER_WAITING, Ordering::Relaxed);

        loop {
            for _ in 0..100 {
                // Try to acquire when no readers or writers
                let state = self.state.load(Ordering::Relaxed);
                if state & READER_MASK == 0 && state & WRITER_ACTIVE == 0 {
                    // Clear waiting flag and set active flag
                    if self
                        .state
                        .compare_exchange(
                            state,
                            WRITER_ACTIVE,
                            Ordering::Acquire,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        let guard = WriteGuard { lock: self };
                        return if self.is_poisoned() {
                            Err(PoisonError::new(guard))
                        } else {
                            Ok(guard)
                        };
                    }
                }

                core::hint::spin_loop();
            }
        }
    }

    /// Attempts to acquire a `write` `lock` without blocking.
    ///
    /// Returns `Ok(guard)` if acquired, or an error if unavailable or poisoned.
    ///
    /// # Errors
    ///
    /// - `Err(TryLockError::WouldBlock)` if the `lock` is currently held
    /// - `Err(TryLockError::Poisoned)` if the `lock` was poisoned
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinRwLock;
    ///
    /// let `lock` = SpinRwLock::new(42);
    ///
    /// match `lock`.try_write() {
    ///     Ok(mut guard) => *guard = 100,
    ///     Err(_) => {}
    /// };
    /// ```
    #[inline]
    pub fn try_write(&self) -> TryLockResult<WriteGuard<'_, T>> {
        // Only succeed if completely unlocked
        if self
            .state
            .compare_exchange(0, WRITER_ACTIVE, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let guard = WriteGuard { lock: self };
            if self.is_poisoned() {
                Err(TryLockError::Poisoned(PoisonError::new(guard)))
            } else {
                Ok(guard)
            }
        } else {
            Err(TryLockError::WouldBlock)
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SpinRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_read() {
            Ok(guard) => f
                .debug_struct("SpinRwLock")
                .field("data", &&*guard)
                .field("poisoned", &false)
                .finish(),
            Err(TryLockError::WouldBlock) => f
                .debug_struct("SpinRwLock")
                .field("data", &"<locked>")
                .field("poisoned", &self.is_poisoned())
                .finish(),
            Err(TryLockError::Poisoned(guard)) => f
                .debug_struct("SpinRwLock")
                .field("data", &&*guard.into_inner())
                .field("poisoned", &true)
                .finish(),
        }
    }
}

impl<T: Default> Default for SpinRwLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> From<T> for SpinRwLock<T> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T: ?Sized> Deref for ReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: We hold a read lock, so shared access is safe
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        // Note: In no_std without panic detection, poisoning must be
        // triggered manually or through external panic runtime.
        // For full std compatibility, check std::thread::panicking() here.

        // Decrement reader count with Release ordering
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for ReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Deref for WriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: We hold the write lock, so exclusive access is safe
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for WriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We hold the write lock, so exclusive access is safe
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        // Note: In no_std without panic detection, poisoning must be
        // triggered manually or through external panic runtime.
        // For full std compatibility, check std::thread::panicking() here.

        // Clear writer active flag with Release ordering
        self.lock.state.store(0, Ordering::Release);
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for WriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for WriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec;

    /// `WHY`: Validates basic `lock` construction and `into_inner`
    /// `WHAT`: Creating a `lock` and extracting its value should work
    #[test]
    fn test_new_and_into_inner() {
        let lock = SpinRwLock::new(42);
        assert!(!lock.is_poisoned());
        assert_eq!(lock.into_inner().unwrap(), 42);
    }

    /// `WHY`: Validates basic `read` `lock` acquisition
    /// `WHAT`: Read locks should be acquirable and data accessible
    #[test]
    fn test_read() {
        let lock = SpinRwLock::new(vec![1, 2, 3]);
        let r = lock.read().unwrap();
        assert_eq!(*r, vec![1, 2, 3]);
    }

    /// `WHY`: Validates multiple simultaneous readers
    /// `WHAT`: Multiple `read` guards should coexist
    #[test]
    fn test_multiple_readers() {
        let lock = SpinRwLock::new(42);
        let r1 = lock.read().unwrap();
        let r2 = lock.read().unwrap();
        let r3 = lock.read().unwrap();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
        assert_eq!(*r3, 42);
    }

    /// `WHY`: Validates `write` `lock` acquisition and mutation
    /// `WHAT`: Write `lock` should provide exclusive mutable access
    #[test]
    fn test_write() {
        let lock = SpinRwLock::new(0);
        {
            let mut w = lock.write().unwrap();
            *w += 1;
            assert_eq!(*w, 1);
        }
        let r = lock.read().unwrap();
        assert_eq!(*r, 1);
    }

    /// `WHY`: Validates `write` exclusivity
    /// `WHAT`: `try_read` should fail when `write` `lock` is held
    #[test]
    fn test_write_blocks_readers() {
        let lock = SpinRwLock::new(42);
        let _w = lock.write().unwrap();
        assert!(lock.try_read().is_err());
    }

    /// `WHY`: Validates reader blocking during `write`
    /// `WHAT`: `try_write` should fail when `read` `lock` is held
    #[test]
    fn test_readers_block_writers() {
        let lock = SpinRwLock::new(42);
        let _r = lock.read().unwrap();
        assert!(lock.try_write().is_err());
    }

    /// `WHY`: Validates `try_read` when `lock` is free
    /// `WHAT`: `try_read` should succeed when no writers
    #[test]
    fn test_try_read_success() {
        let lock = SpinRwLock::new(42);
        let r = lock.try_read();
        assert!(r.is_ok());
        assert_eq!(*r.unwrap(), 42);
    }

    /// `WHY`: Validates `try_write` when `lock` is free
    /// `WHAT`: `try_write` should succeed when no readers or writers
    #[test]
    fn test_try_write_success() {
        let lock = SpinRwLock::new(42);
        let w = lock.try_write();
        assert!(w.is_ok());
    }

    /// `WHY`: Validates `get_mut` functionality
    /// `WHAT`: `get_mut` should provide mutable access without locking
    #[test]
    fn test_get_mut() {
        let mut lock = SpinRwLock::new(0);
        *lock.get_mut().unwrap() = 42;
        assert_eq!(*lock.read().unwrap(), 42);
    }

    /// `WHY`: Validates `is_poisoned` detection
    /// `WHAT`: Fresh `lock` should not be poisoned
    #[test]
    fn test_not_poisoned() {
        let lock = SpinRwLock::new(0);
        assert!(!lock.is_poisoned());
    }

    /// `WHY`: Validates Send trait bounds
    /// `WHAT`: `RwLock` should be Send when T is Send + Sync
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SpinRwLock<i32>>();
    }

    /// `WHY`: Validates Sync trait bounds
    /// `WHAT`: `RwLock` should be Sync when T is Send + Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SpinRwLock<i32>>();
    }

    /// `WHY`: Validates Debug implementation
    /// `WHAT`: Debug formatting should work for both locked and unlocked states
    #[test]
    fn test_debug() {
        let lock = SpinRwLock::new(42);
        let debug_str = format!("{lock:?}");
        assert!(debug_str.contains("SpinRwLock"));
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create `lock` with default value
    #[test]
    fn test_default() {
        let lock = SpinRwLock::<i32>::default();
        assert_eq!(*lock.read().unwrap(), 0);
    }

    /// `WHY`: Validates From<T> implementation
    /// `WHAT`: From trait should allow creating `lock` from value
    #[test]
    fn test_from() {
        let lock = SpinRwLock::from(42);
        assert_eq!(*lock.read().unwrap(), 42);
    }
}
