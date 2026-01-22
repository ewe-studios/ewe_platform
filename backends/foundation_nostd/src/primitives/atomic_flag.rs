//! Simple atomic boolean flag.
//!
//! This provides a simpler API than `AtomicBool` for common flag patterns.
//! It's useful for one-shot flags, shutdown signals, and simple state tracking.
//!
//! # Examples
//!
//! ```
//! use foundation_nostd::primitives::AtomicFlag;
//!
//! let flag = AtomicFlag::new(false);
//! assert!(!flag.is_set());
//!
//! flag.set();
//! assert!(flag.is_set());
//!
//! flag.clear();
//! assert!(!flag.is_set());
//! ```

use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple atomic boolean flag.
///
/// This provides set/clear/is_set operations with sensible default
/// memory ordering. For more control, use `AtomicBool` directly.
pub struct AtomicFlag {
    inner: AtomicBool,
}

impl AtomicFlag {
    /// Creates a new `AtomicFlag` with the given initial value.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(false);
    /// assert!(!flag.is_set());
    ///
    /// let flag = AtomicFlag::new(true);
    /// assert!(flag.is_set());
    /// ```
    #[inline]
    pub const fn new(initial: bool) -> Self {
        Self {
            inner: AtomicBool::new(initial),
        }
    }

    /// Checks if the flag is set.
    ///
    /// Uses `Acquire` ordering to ensure visibility of writes from other threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(true);
    /// assert!(flag.is_set());
    /// ```
    #[inline]
    pub fn is_set(&self) -> bool {
        self.inner.load(Ordering::Acquire)
    }

    /// Sets the flag to `true`.
    ///
    /// Uses `Release` ordering to ensure all previous writes are visible
    /// to threads that observe the flag being set.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(false);
    /// flag.set();
    /// assert!(flag.is_set());
    /// ```
    #[inline]
    pub fn set(&self) {
        self.inner.store(true, Ordering::Release);
    }

    /// Clears the flag to `false`.
    ///
    /// Uses `Release` ordering to ensure all previous writes are visible.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(true);
    /// flag.clear();
    /// assert!(!flag.is_set());
    /// ```
    #[inline]
    pub fn clear(&self) {
        self.inner.store(false, Ordering::Release);
    }

    /// Sets the flag and returns the previous value.
    ///
    /// Uses `AcqRel` ordering for both acquire and release semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(false);
    /// assert!(!flag.test_and_set());
    /// assert!(flag.is_set());
    /// ```
    #[inline]
    pub fn test_and_set(&self) -> bool {
        self.inner.swap(true, Ordering::AcqRel)
    }

    /// Clears the flag and returns the previous value.
    ///
    /// Uses `AcqRel` ordering for both acquire and release semantics.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(true);
    /// assert!(flag.test_and_clear());
    /// assert!(!flag.is_set());
    /// ```
    #[inline]
    pub fn test_and_clear(&self) -> bool {
        self.inner.swap(false, Ordering::AcqRel)
    }

    /// Performs a compare-and-swap operation.
    ///
    /// Sets the flag to `new` if the current value is `current`.
    /// Returns `Ok(current)` if successful, or `Err(actual)` with the actual
    /// value if the comparison failed.
    ///
    /// Uses `AcqRel` ordering for success and `Acquire` for failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(false);
    ///
    /// // Successful CAS
    /// assert_eq!(flag.compare_and_swap(false, true), Ok(false));
    /// assert!(flag.is_set());
    ///
    /// // Failed CAS - flag is already true
    /// assert_eq!(flag.compare_and_swap(false, true), Err(true));
    /// ```
    #[inline]
    pub fn compare_and_swap(&self, current: bool, new: bool) -> Result<bool, bool> {
        self.inner
            .compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
    }

    /// Loads the current value with relaxed ordering.
    ///
    /// This is faster but provides no synchronization guarantees.
    /// Use only when ordering doesn't matter (e.g., statistics).
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(true);
    /// assert!(flag.load_relaxed());
    /// ```
    #[inline]
    pub fn load_relaxed(&self) -> bool {
        self.inner.load(Ordering::Relaxed)
    }

    /// Stores a value with relaxed ordering.
    ///
    /// This is faster but provides no synchronization guarantees.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::AtomicFlag;
    ///
    /// let flag = AtomicFlag::new(false);
    /// flag.store_relaxed(true);
    /// assert!(flag.load_relaxed());
    /// ```
    #[inline]
    pub fn store_relaxed(&self, value: bool) {
        self.inner.store(value, Ordering::Relaxed);
    }
}

impl Default for AtomicFlag {
    /// Creates a new `AtomicFlag` initialized to `false`.
    #[inline]
    fn default() -> Self {
        Self::new(false)
    }
}

impl fmt::Debug for AtomicFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AtomicFlag")
            .field("value", &self.is_set())
            .finish()
    }
}

impl From<bool> for AtomicFlag {
    #[inline]
    fn from(value: bool) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    /// WHY: Validates AtomicFlag construction and initial state
    /// WHAT: Creating a flag should set initial value correctly
    #[test]
    fn test_new() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.is_set());

        let flag = AtomicFlag::new(true);
        assert!(flag.is_set());
    }

    /// WHY: Validates set operation
    /// WHAT: set() should change flag to true
    #[test]
    fn test_set() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.is_set());

        flag.set();
        assert!(flag.is_set());

        // Setting again should be idempotent
        flag.set();
        assert!(flag.is_set());
    }

    /// WHY: Validates clear operation
    /// WHAT: clear() should change flag to false
    #[test]
    fn test_clear() {
        let flag = AtomicFlag::new(true);
        assert!(flag.is_set());

        flag.clear();
        assert!(!flag.is_set());

        // Clearing again should be idempotent
        flag.clear();
        assert!(!flag.is_set());
    }

    /// WHY: Validates test_and_set atomic operation
    /// WHAT: Should return old value and set to true
    #[test]
    fn test_test_and_set() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.test_and_set());
        assert!(flag.is_set());

        // Second call should return true
        assert!(flag.test_and_set());
        assert!(flag.is_set());
    }

    /// WHY: Validates test_and_clear atomic operation
    /// WHAT: Should return old value and set to false
    #[test]
    fn test_test_and_clear() {
        let flag = AtomicFlag::new(true);
        assert!(flag.test_and_clear());
        assert!(!flag.is_set());

        // Second call should return false
        assert!(!flag.test_and_clear());
        assert!(!flag.is_set());
    }

    /// WHY: Validates compare_and_swap success case
    /// WHAT: CAS should succeed when current value matches
    #[test]
    fn test_compare_and_swap_success() {
        let flag = AtomicFlag::new(false);

        assert_eq!(flag.compare_and_swap(false, true), Ok(false));
        assert!(flag.is_set());

        assert_eq!(flag.compare_and_swap(true, false), Ok(true));
        assert!(!flag.is_set());
    }

    /// WHY: Validates compare_and_swap failure case
    /// WHAT: CAS should fail when current value doesn't match
    #[test]
    fn test_compare_and_swap_failure() {
        let flag = AtomicFlag::new(false);

        assert_eq!(flag.compare_and_swap(true, false), Err(false));
        assert!(!flag.is_set());
    }

    /// WHY: Validates relaxed load operation
    /// WHAT: load_relaxed should return current value
    #[test]
    fn test_load_relaxed() {
        let flag = AtomicFlag::new(true);
        assert!(flag.load_relaxed());

        flag.clear();
        assert!(!flag.load_relaxed());
    }

    /// WHY: Validates relaxed store operation
    /// WHAT: store_relaxed should set value
    #[test]
    fn test_store_relaxed() {
        let flag = AtomicFlag::new(false);

        flag.store_relaxed(true);
        assert!(flag.load_relaxed());

        flag.store_relaxed(false);
        assert!(!flag.load_relaxed());
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should create flag initialized to false
    #[test]
    fn test_default() {
        let flag = AtomicFlag::default();
        assert!(!flag.is_set());
    }

    /// WHY: Validates Debug implementation
    /// WHAT: Debug formatting should show current value
    #[test]
    fn test_debug() {
        let flag = AtomicFlag::new(true);
        let debug_str = format!("{:?}", flag);
        assert!(debug_str.contains("AtomicFlag"));
        assert!(debug_str.contains("true"));
    }

    /// WHY: Validates From<bool> implementation
    /// WHAT: From trait should create flag from bool
    #[test]
    fn test_from_bool() {
        let flag = AtomicFlag::from(true);
        assert!(flag.is_set());

        let flag = AtomicFlag::from(false);
        assert!(!flag.is_set());
    }
}
