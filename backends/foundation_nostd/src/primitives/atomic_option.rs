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

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Validates basic atomic option creation
    /// WHAT: New none option should be empty
    #[test]
    fn test_none() {
        let opt = AtomicOption::<i32>::none();
        assert!(opt.is_none());
        assert!(!opt.is_some());
    }

    /// WHY: Validates some constructor
    /// WHAT: New some option should contain value
    #[test]
    fn test_some() {
        let opt = AtomicOption::some(42);
        assert!(opt.is_some());
        assert!(!opt.is_none());
    }

    /// WHY: Validates take removes the value
    /// WHAT: After take, option should be empty
    #[test]
    fn test_take() {
        let opt = AtomicOption::some(42);
        assert_eq!(opt.take(), Some(42));
        assert!(opt.is_none());
    }

    /// WHY: Validates take on empty option
    /// WHAT: Taking from empty option should return None
    #[test]
    fn test_take_none() {
        let opt = AtomicOption::<i32>::none();
        assert_eq!(opt.take(), None);
    }

    /// WHY: Validates swap replaces the value
    /// WHAT: Swap should return old value and store new
    #[test]
    fn test_swap() {
        let opt = AtomicOption::some(1);
        let old = opt.swap(Some(2));
        assert_eq!(old, Some(1));
        assert_eq!(opt.load(), Some(2));
    }

    /// WHY: Validates swap with None
    /// WHAT: Swap with None should empty the option
    #[test]
    fn test_swap_none() {
        let opt = AtomicOption::some(42);
        let old = opt.swap(None);
        assert_eq!(old, Some(42));
        assert!(opt.is_none());
    }

    /// WHY: Validates store updates the value
    /// WHAT: After store, load should return new value
    #[test]
    fn test_store() {
        let opt = AtomicOption::none();
        opt.store(Some(42));
        assert_eq!(opt.load(), Some(42));
    }

    /// WHY: Validates load returns the value
    /// WHAT: Load should return current value
    #[test]
    fn test_load() {
        let opt = AtomicOption::some(42);
        assert_eq!(opt.load(), Some(42));
    }

    /// WHY: Validates get_mut provides mutable access
    /// WHAT: get_mut should allow direct mutation and into_inner should reflect it
    #[test]
    fn test_get_mut() {
        let mut opt = AtomicOption::none();
        *opt.get_mut() = Some(10);
        // Note: get_mut bypasses the atomic flag, so load() may not see the change
        // but into_inner() will
        assert_eq!(opt.into_inner(), Some(10));
    }

    /// WHY: Validates into_inner consumes and returns value
    /// WHAT: Should return the inner option
    #[test]
    fn test_into_inner() {
        let opt = AtomicOption::some(42);
        assert_eq!(opt.into_inner(), Some(42));
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should create empty option
    #[test]
    fn test_default() {
        let opt = AtomicOption::<i32>::default();
        assert!(opt.is_none());
    }

    /// WHY: Validates From implementation
    /// WHAT: From should create option from Option<T>
    #[test]
    fn test_from() {
        let opt = AtomicOption::from(Some(42));
        assert_eq!(opt.load(), Some(42));
    }

    /// WHY: Validates Send bound requirement
    /// WHAT: AtomicOption should be Send when T: Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AtomicOption<i32>>();
    }

    /// WHY: Validates Sync bound requirement
    /// WHAT: AtomicOption should be Sync when T: Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AtomicOption<i32>>();
    }
}
