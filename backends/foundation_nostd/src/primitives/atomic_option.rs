//! Atomic Option type for optional values.
//!
//! This provides atomic operations on `Option<T>` for `Copy` types.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// An atomic optional value.
///
/// This allows atomically storing and retrieving optional values.
pub struct AtomicOption<T: Copy> {
    has_value: AtomicBool,
    value: UnsafeCell<Option<T>>,
}

unsafe impl<T: Copy + Send> Send for AtomicOption<T> {}
unsafe impl<T: Copy + Send> Sync for AtomicOption<T> {}

impl<T: Copy> AtomicOption<T> {
    /// Creates a new empty atomic option.
    #[inline]
    pub const fn none() -> Self {
        Self {
            has_value: AtomicBool::new(false),
            value: UnsafeCell::new(None),
        }
    }

    /// Creates a new atomic option with a value.
    #[inline]
    pub const fn some(value: T) -> Self {
        Self {
            has_value: AtomicBool::new(true),
            value: UnsafeCell::new(Some(value)),
        }
    }

    /// Returns `true` if the option contains a value.
    #[inline]
    pub fn is_some(&self) -> bool {
        self.has_value.load(Ordering::Acquire)
    }

    /// Returns `true` if the option is empty.
    #[inline]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }

    /// Takes the value, leaving the option empty.
    #[inline]
    pub fn take(&self) -> Option<T> {
        if self.has_value.swap(false, Ordering::AcqRel) {
            unsafe { (*self.value.get()).take() }
        } else {
            None
        }
    }

    /// Swaps the value with a new option, returning the old value.
    #[inline]
    pub fn swap(&self, new_value: Option<T>) -> Option<T> {
        let old = self.take();
        if let Some(v) = new_value {
            self.has_value.store(true, Ordering::Release);
            unsafe {
                *self.value.get() = Some(v);
            }
        }
        old
    }

    /// Stores a value.
    #[inline]
    pub fn store(&self, value: Option<T>) {
        unsafe {
            *self.value.get() = value;
        }
        self.has_value.store(value.is_some(), Ordering::Release);
    }

    /// Loads the value.
    #[inline]
    pub fn load(&self) -> Option<T> {
        if self.has_value.load(Ordering::Acquire) {
            unsafe { *self.value.get() }
        } else {
            None
        }
    }

    /// Gets a mutable reference to the inner option.
    #[inline]
    pub fn get_mut(&mut self) -> &mut Option<T> {
        self.value.get_mut()
    }

    /// Consumes the atomic option and returns the inner value.
    #[inline]
    pub fn into_inner(self) -> Option<T> {
        self.value.into_inner()
    }
}

impl<T: Copy> Default for AtomicOption<T> {
    #[inline]
    fn default() -> Self {
        Self::none()
    }
}

impl<T: Copy> From<Option<T>> for AtomicOption<T> {
    #[inline]
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => Self::some(v),
            None => Self::none(),
        }
    }
}
