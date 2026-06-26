//! Lazy atomic initialization.
//!
//! This provides thread-safe lazy initialization for static values.

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::MaybeUninit;
use core::ops::Deref;

use crate::primitives::Once;

/// A value that is lazily initialized on first access.
///
/// This is useful for static values that require runtime initialization.
pub struct AtomicLazy<T, F = fn() -> T> {
    once: Once,
    data: UnsafeCell<MaybeUninit<T>>,
    init: UnsafeCell<Option<F>>,
}

unsafe impl<T: Send, F: Send> Send for AtomicLazy<T, F> {}
unsafe impl<T: Sync, F: Send> Sync for AtomicLazy<T, F> {}

impl<T, F: FnOnce() -> T> AtomicLazy<T, F> {
    /// Creates a new lazy value with the given initializer.
    #[inline]
    pub const fn new(init: F) -> Self {
        Self {
            once: Once::new(),
            data: UnsafeCell::new(MaybeUninit::uninit()),
            init: UnsafeCell::new(Some(init)),
        }
    }

    /// Forces initialization and returns a reference to the value.
    ///
    /// # Panics
    ///
    /// Panics if the initializer function has already been called or taken.
    /// This should not happen in normal usage.
    #[inline]
    pub fn force(this: &Self) -> &T {
        this.once.call_once(|| {
            let init = unsafe { (*this.init.get()).take().unwrap() };
            let value = init();
            unsafe {
                (*this.data.get()).write(value);
            }
        });

        unsafe { (*this.data.get()).assume_init_ref() }
    }
}

impl<T, F: FnOnce() -> T> Deref for AtomicLazy<T, F> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}

impl<T: Default> Default for AtomicLazy<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default)
    }
}

impl<T: fmt::Debug, F: FnOnce() -> T> fmt::Debug for AtomicLazy<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtomicLazy")
            .field("value", &**self)
            .finish()
    }
}

impl<T, F> Drop for AtomicLazy<T, F> {
    fn drop(&mut self) {
        if self.once.is_completed() {
            unsafe {
                self.data.get_mut().assume_init_drop();
            }
        }
    }
}
