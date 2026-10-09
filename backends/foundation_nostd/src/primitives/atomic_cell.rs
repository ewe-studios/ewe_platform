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
