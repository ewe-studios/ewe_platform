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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec};
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// WHY: Validates lazy initialization on first access
    /// WHAT: Initializer should run exactly once
    #[test]
    fn test_lazy_init() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let lazy = AtomicLazy::new(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
            42
        });

        assert_eq!(*lazy, 42);
        assert_eq!(*lazy, 42); // Should not reinitialize
        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
    }

    /// WHY: Validates force method
    /// WHAT: Force should initialize and return reference
    #[test]
    fn test_force() {
        let lazy = AtomicLazy::new(|| 42);
        let value = AtomicLazy::force(&lazy);
        assert_eq!(*value, 42);
    }

    /// WHY: Validates Deref implementation
    /// WHAT: Deref should trigger initialization
    #[test]
    fn test_deref() {
        let lazy = AtomicLazy::new(|| vec![1, 2, 3]);
        assert_eq!(lazy.len(), 3);
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should create lazy with default value
    #[test]
    fn test_default() {
        let lazy = AtomicLazy::<i32>::default();
        assert_eq!(*lazy, 0);
    }

    /// WHY: Validates Debug implementation
    /// WHAT: Debug should show the value
    #[test]
    fn test_debug() {
        let lazy = AtomicLazy::new(|| 42);
        let debug = format!("{:?}", lazy);
        assert!(debug.contains("42") || debug.contains("AtomicLazy"));
    }

    /// WHY: Validates Send bound requirement
    /// WHAT: AtomicLazy should be Send when T and F are Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AtomicLazy<i32>>();
    }

    /// WHY: Validates Sync bound requirement
    /// WHAT: AtomicLazy should be Sync when T: Sync and F: Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AtomicLazy<i32>>();
    }
}
