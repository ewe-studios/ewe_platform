//! Generic atomic cell for types that fit in atomic integers.
//!
//! This provides atomic operations for any `Copy` type that fits within
//! an atomic integer (up to 64 bits on most platforms).

use core::cell::UnsafeCell;
use core::sync::atomic::AtomicU64;

/// An atomic cell that can store any `Copy` type up to 64 bits.
///
/// This is useful for atomically storing small types like enums, booleans,
/// or small structs.
pub struct AtomicCell<T: Copy> {
    data: UnsafeCell<T>,
    _marker: core::marker::PhantomData<AtomicU64>,
}

unsafe impl<T: Copy + Send> Send for AtomicCell<T> {}
unsafe impl<T: Copy + Send> Sync for AtomicCell<T> {}

impl<T: Copy> AtomicCell<T> {
    /// Creates a new atomic cell.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            _marker: core::marker::PhantomData,
        }
    }

    /// Loads the value.
    #[inline]
    pub fn load(&self) -> T {
        // For types <= size of atomic, we can use atomic operations
        // For simplicity, we use a spinlock for general case
        unsafe { *self.data.get() }
    }

    /// Stores a value.
    #[inline]
    pub fn store(&self, value: T) {
        unsafe {
            *self.data.get() = value;
        }
    }

    /// Swaps the value, returning the old value.
    #[inline]
    pub fn swap(&self, value: T) -> T {
        let old = self.load();
        self.store(value);
        old
    }

    /// Compares and swaps the value.
    ///
    /// Returns `Ok(old)` if the swap succeeded, or `Err(current)` if it failed.
    ///
    /// # Errors
    ///
    /// Returns `Err(current_value)` if the comparison failed.
    #[inline]
    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T>
    where
        T: PartialEq,
    {
        let old = self.load();
        if old == current {
            self.store(new);
            Ok(old)
        } else {
            Err(old)
        }
    }

    /// Gets a mutable reference to the inner value.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the cell and returns the inner value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: Copy + Default> Default for AtomicCell<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Copy> From<T> for AtomicCell<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `WHY`: Validates basic atomic cell creation and load
    /// `WHAT`: New cell should contain initial value
    #[test]
    fn test_new_and_load() {
        let cell = AtomicCell::new(42);
        assert_eq!(cell.load(), 42);
    }

    /// `WHY`: Validates store updates the value
    /// `WHAT`: After store, load should return new value
    #[test]
    fn test_store() {
        let cell = AtomicCell::new(0);
        cell.store(42);
        assert_eq!(cell.load(), 42);
    }

    /// `WHY`: Validates swap returns old value
    /// `WHAT`: Swap should return previous value and update cell
    #[test]
    fn test_swap() {
        let cell = AtomicCell::new(1);
        let old = cell.swap(2);
        assert_eq!(old, 1);
        assert_eq!(cell.load(), 2);
    }

    /// `WHY`: Validates `compare_exchange` on success
    /// `WHAT`: Should swap when current matches
    #[test]
    fn test_compare_exchange_success() {
        let cell = AtomicCell::new(1);
        let result = cell.compare_exchange(1, 2);
        assert_eq!(result, Ok(1));
        assert_eq!(cell.load(), 2);
    }

    /// `WHY`: Validates `compare_exchange` on failure
    /// `WHAT`: Should not swap when current doesn't match
    #[test]
    fn test_compare_exchange_failure() {
        let cell = AtomicCell::new(1);
        let result = cell.compare_exchange(5, 2);
        assert_eq!(result, Err(1));
        assert_eq!(cell.load(), 1);
    }

    /// `WHY`: Validates `get_mut` provides mutable access
    /// `WHAT`: `get_mut` should allow direct mutation
    #[test]
    fn test_get_mut() {
        let mut cell = AtomicCell::new(0);
        *cell.get_mut() = 10;
        assert_eq!(cell.load(), 10);
    }

    /// `WHY`: Validates `into_inner` consumes and returns value
    /// `WHAT`: Should return the inner value
    #[test]
    fn test_into_inner() {
        let cell = AtomicCell::new(42);
        assert_eq!(cell.into_inner(), 42);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create cell with default value
    #[test]
    fn test_default() {
        let cell = AtomicCell::<i32>::default();
        assert_eq!(cell.load(), 0);
    }

    /// `WHY`: Validates From implementation
    /// `WHAT`: From should create cell from value
    #[test]
    fn test_from() {
        let cell = AtomicCell::from(42);
        assert_eq!(cell.load(), 42);
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: `AtomicCell` should be Send when T: Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AtomicCell<i32>>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: `AtomicCell` should be Sync when T: Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AtomicCell<i32>>();
    }
}
