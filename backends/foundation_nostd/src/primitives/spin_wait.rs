//! Exponential backoff for spin waiting.
//!
//! This provides a helper for efficient spin-waiting with exponential backoff.
//! Instead of constantly spinning, it gradually increases the wait time to
//! reduce CPU usage while remaining responsive.
//!
//! # Examples
//!
//! ```
//! use foundation_nostd::primitives::SpinWait;
//!
//! let mut spin = SpinWait::new();
//!
//! // Try to acquire a lock
//! loop {
//!     if try_acquire_lock() {
//!         break;
//!     }
//!
//!     if !spin.spin() {
//!         // Yielded maximum times, consider blocking
//!         break;
//!     }
//! }
//! # fn try_acquire_lock() -> bool { true }
//! ```

use core::hint;

/// Exponential backoff for spin-waiting.
///
/// This gradually increases the number of iterations in each `spin` cycle,
/// reducing CPU usage while maintaining responsiveness for short waits.
///
/// # Algorithm
///
/// - First few spins: Just `spin_loop` hints to the CPU
/// - After threshold: Exponentially increase `spin` count
/// - After max: Return `false` to indicate caller should yield/block
pub struct SpinWait {
    counter: u32,
}

// Configuration constants
const SPIN_LIMIT: u32 = 10; // Number of exponential backoff iterations

impl SpinWait {
    /// Creates a new `SpinWait` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWait;
    ///
    /// let mut `spin` = SpinWait::new();
    /// ```
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { counter: 0 }
    }

    /// Performs one `spin` iteration with exponential backoff.
    ///
    /// Returns `true` if spinning should continue, or `false` if the
    /// maximum `spin` count has been reached and the caller should consider
    /// yielding or blocking.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWait;
    ///
    /// let mut `spin` = SpinWait::new();
    /// while `spin`.spin() {
    ///     // Keep spinning
    /// }
    /// ```
    #[inline]
    pub fn spin(&mut self) -> bool {
        if self.counter >= SPIN_LIMIT {
            // Exceeded maximum spin iterations
            return false;
        }

        // Exponential backoff: 1, 2, 4, 8, 16, ..., MAX_SPIN
        let spins = 1u32 << self.counter.min(31); // Clamp to avoid overflow

        for _ in 0..spins {
            // Use CPU-specific spin-wait hint
            hint::spin_loop();
        }

        self.counter += 1;
        true
    }

    /// Spins exactly once without incrementing the `counter`.
    ///
    /// This is useful when you want to `spin` without the exponential backoff.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWait;
    ///
    /// let mut `spin` = SpinWait::new();
    /// `spin`.spin_once();
    /// ```
    #[inline]
    pub fn spin_once(&self) {
        hint::spin_loop();
    }

    /// Resets the `spin` wait `counter` to zero.
    ///
    /// This allows reusing the `SpinWait` instance for a new `spin` sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWait;
    ///
    /// let mut `spin` = SpinWait::new();
    /// while `spin`.spin() {
    ///     // Spinning...
    /// }
    /// `spin`.reset();  // Start fresh
    /// ```
    #[inline]
    pub fn reset(&mut self) {
        self.counter = 0;
    }

    /// Returns the current `spin` `counter` value.
    ///
    /// This indicates how many times `spin()` has been called successfully.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWait;
    ///
    /// let mut `spin` = SpinWait::new();
    /// assert_eq!(spin.counter(), 0);
    /// `spin`.spin();
    /// assert_eq!(spin.counter(), 1);
    /// ```
    #[inline]
    #[must_use]
    pub fn counter(&self) -> u32 {
        self.counter
    }

    /// Returns whether the spinner has reached its limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWait;
    ///
    /// let mut `spin` = SpinWait::new();
    /// assert!(!spin.is_exhausted());
    /// while `spin`.spin() {}
    /// assert!(spin.is_exhausted());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.counter >= SPIN_LIMIT
    }
}

impl Default for SpinWait {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `WHY`: Validates `SpinWait` construction
    /// `WHAT`: Creating a `SpinWait` should start with `counter` at 0
    #[test]
    fn test_new() {
        let spin = SpinWait::new();
        assert_eq!(spin.counter(), 0);
        assert!(!spin.is_exhausted());
    }

    /// `WHY`: Validates `spin` progression
    /// `WHAT`: Each `spin()` call should increment `counter` until exhausted
    #[test]
    fn test_spin_progression() {
        let mut spin = SpinWait::new();
        let mut count = 0;

        while spin.spin() {
            count += 1;
        }

        assert!(count > 0);
        assert!(spin.is_exhausted());
    }

    /// `WHY`: Validates `spin` returns false when exhausted
    /// `WHAT`: `spin()` should return false after `SPIN_LIMIT` iterations
    #[test]
    fn test_spin_exhaustion() {
        let mut spin = SpinWait::new();

        for _ in 0..SPIN_LIMIT {
            assert!(spin.spin());
        }

        assert!(!spin.spin());
        assert!(spin.is_exhausted());
    }

    /// `WHY`: Validates `reset` functionality
    /// `WHAT`: `reset()` should allow restarting `spin` sequence
    #[test]
    fn test_reset() {
        let mut spin = SpinWait::new();

        while spin.spin() {}
        assert!(spin.is_exhausted());

        spin.reset();
        assert_eq!(spin.counter(), 0);
        assert!(!spin.is_exhausted());
        assert!(spin.spin());
    }

    /// `WHY`: Validates `spin_once` doesn't affect `counter`
    /// `WHAT`: `spin_once()` should `spin` without incrementing `counter`
    #[test]
    fn test_spin_once() {
        let spin = SpinWait::new();
        assert_eq!(spin.counter(), 0);

        spin.spin_once();
        assert_eq!(spin.counter(), 0);

        spin.spin_once();
        assert_eq!(spin.counter(), 0);
    }

    /// `WHY`: Validates `counter` tracking
    /// `WHAT`: `counter()` should return number of successful spins
    #[test]
    fn test_counter() {
        let mut spin = SpinWait::new();
        assert_eq!(spin.counter(), 0);

        spin.spin();
        assert_eq!(spin.counter(), 1);

        spin.spin();
        assert_eq!(spin.counter(), 2);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create fresh `SpinWait`
    #[test]
    fn test_default() {
        let spin = SpinWait::default();
        assert_eq!(spin.counter(), 0);
        assert!(!spin.is_exhausted());
    }
}
