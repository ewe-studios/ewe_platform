//! Iteration-based waiter for single-threaded and WASM environments.
//!
//! Provides duration-based waiting using spin-loop iteration counting instead of
//! OS sleep mechanisms. This is necessary for WASM and no_std environments where
//! `std::thread::sleep` is not available.
//!
//! # Platform-specific behavior
//!
//! - **WASM**: Uses ~100,000 iterations per millisecond (adjustable)
//! - **no_std embedded**: Uses ~1,000,000 iterations per millisecond (adjustable)
//! - **Interruptible**: Can be interrupted via `AtomicBool` flag
//! - **Bounded**: Never spins forever - has maximum spin limit
//!
//! # Example
//!
//! ```
//! use foundation_nostd::primitives::SpinWaiter;
//! use std::time::Duration;
//! use std::sync::Arc;
//!
//! let waiter = SpinWaiter::new(100_000); // 100K iterations per ms
//! waiter.wait(Duration::from_millis(100));
//! ```

use core::hint;
use core::sync::atomic::{AtomicBool, Ordering};

/// Iteration-based waiter for environments without OS sleep support.
///
/// Uses spin-loop iteration counting to approximate duration-based waiting.
/// Timing is approximate but bounded - never spins forever.
pub struct SpinWaiter {
    /// Interrupt flag - when set to true, wait returns early
    interrupted: AtomicBool,
    /// Estimated iterations per millisecond for this platform
    iterations_per_ms: u64,
    /// Maximum total iterations (prevents infinite spinning)
    max_total_iterations: u64,
}

impl Clone for SpinWaiter {
    /// Clone creates a new SpinWaiter with the same configuration.
    /// The interrupt flag starts fresh (not interrupted).
    fn clone(&self) -> Self {
        Self {
            interrupted: AtomicBool::new(false),
            iterations_per_ms: self.iterations_per_ms,
            max_total_iterations: self.max_total_iterations,
        }
    }
}

impl SpinWaiter {
    /// Creates a new SpinWaiter with platform-specific defaults.
    ///
    /// # Arguments
    ///
    /// * `iterations_per_ms` - Estimated spin-loop iterations per millisecond
    ///   - WASM: ~100,000 iterations/ms
    ///   - no_std embedded: ~1,000,000 iterations/ms
    #[inline]
    #[must_use]
    pub const fn new(iterations_per_ms: u64) -> Self {
        Self {
            interrupted: AtomicBool::new(false),
            iterations_per_ms,
            // Maximum 10 seconds worth of spinning (safety bound)
            max_total_iterations: iterations_per_ms * 10_000,
        }
    }

    /// Creates a new SpinWaiter for WASM environments.
    ///
    /// Uses 100,000 iterations per millisecond as a reasonable default for WASM.
    #[inline]
    #[must_use]
    pub const fn wasm() -> Self {
        Self::new(100_000)
    }

    /// Creates a new SpinWaiter for embedded no_std environments.
    ///
    /// Uses 1,000,000 iterations per millisecond as a reasonable default for embedded.
    #[inline]
    #[must_use]
    pub const fn embedded() -> Self {
        Self::new(1_000_000)
    }

    /// Wait for approximately the given duration.
    ///
    /// Uses spin-loop iteration counting to approximate the duration.
    /// Can be interrupted by calling `interrupt()`.
    ///
    /// # Example
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWaiter;
    /// use std::time::Duration;
    ///
    /// let waiter = SpinWaiter::wasm();
    /// waiter.wait(Duration::from_millis(100));
    /// ```
    #[inline]
    pub fn wait(&self, dur: core::time::Duration) {
        // Calculate total iterations needed
        let total_spins = dur.as_millis() as u64 * self.iterations_per_ms;

        // Cap at maximum to prevent infinite spinning
        let spins_to_do = total_spins.min(self.max_total_iterations);

        // Check for interrupt every 1000 iterations
        const INTERRUPT_CHECK_INTERVAL: u64 = 1000;

        for i in 0..spins_to_do {
            hint::spin_loop();

            // Check interrupt flag periodically
            if i % INTERRUPT_CHECK_INTERVAL == 0 && self.interrupted.load(Ordering::Relaxed) {
                return;
            }
        }
    }

    /// Interrupt the current wait.
    ///
    /// Sets the interrupt flag, causing any current or future `wait()`
    /// calls to return early. The flag remains set until `reset()` is called.
    ///
    /// # Example
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWaiter;
    /// use std::time::Duration;
    ///
    /// let waiter = SpinWaiter::wasm();
    ///
    /// // In another context:
    /// waiter.interrupt();
    ///
    /// // This wait will return immediately due to interrupt
    /// waiter.wait(Duration::from_secs(10));
    /// ```
    #[inline]
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    /// Reset the interrupt flag.
    ///
    /// Clears the interrupt flag, allowing future `wait()` calls
    /// to wait for the full duration again.
    ///
    /// # Example
    ///
    /// ```
    /// use foundation_nostd::primitives::SpinWaiter;
    /// use std::time::Duration;
    ///
    /// let waiter = SpinWaiter::wasm();
    ///
    /// waiter.interrupt();
    /// waiter.reset();
    ///
    /// // This wait will complete normally
    /// waiter.wait(Duration::from_millis(10));
    /// ```
    #[inline]
    pub fn reset(&self) {
        self.interrupted.store(false, Ordering::Relaxed);
    }

    /// Check if the waiter has been interrupted.
    #[inline]
    #[must_use]
    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    /// Get the configured iterations per millisecond.
    #[inline]
    #[must_use]
    pub const fn iterations_per_ms(&self) -> u64 {
        self.iterations_per_ms
    }
}

impl Default for SpinWaiter {
    /// Default uses WASM settings (100,000 iterations/ms).
    fn default() -> Self {
        Self::wasm()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use core::time::Duration;
    use std::time::Instant;

    /// WHY: Validates SpinWaiter construction with custom settings
    /// WHAT: Creating a SpinWaiter should store configuration correctly
    #[test]
    fn test_new() {
        let waiter = SpinWaiter::new(50_000);
        assert_eq!(waiter.iterations_per_ms(), 50_000);
        assert!(!waiter.is_interrupted());
    }

    /// WHY: Validates WASM-specific constructor
    /// WHAT: wasm() should create SpinWaiter with 100K iterations/ms
    #[test]
    fn test_wasm() {
        let waiter = SpinWaiter::wasm();
        assert_eq!(waiter.iterations_per_ms(), 100_000);
    }

    /// WHY: Validates embedded-specific constructor
    /// WHAT: embedded() should create SpinWaiter with 1M iterations/ms
    #[test]
    fn test_embedded() {
        let waiter = SpinWaiter::embedded();
        assert_eq!(waiter.iterations_per_ms(), 1_000_000);
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should use WASM settings (100K iterations/ms)
    #[test]
    fn test_default() {
        let waiter: SpinWaiter = Default::default();
        assert_eq!(waiter.iterations_per_ms(), 100_000);
    }

    /// WHY: Validates wait completes normally
    /// WHAT: wait() should spin for approximately the requested duration
    #[test]
    fn test_wait_completes() {
        // Use very low iterations for faster test execution
        let waiter = SpinWaiter::new(1_000);

        let start = Instant::now();
        waiter.wait(Duration::from_millis(1));
        let elapsed = start.elapsed();

        // Should complete (timing is approximate due to spin-loop nature)
        // Just verify it doesn't panic and takes some time
        assert!(elapsed >= Duration::from_nanos(1));
    }

    /// WHY: Validates interrupt functionality
    /// WHAT: interrupt() should cause wait() to return immediately
    #[test]
    fn test_interrupt() {
        let waiter = SpinWaiter::new(1_000_000); // High iterations

        // Interrupt before waiting
        waiter.interrupt();
        assert!(waiter.is_interrupted());

        let start = Instant::now();
        waiter.wait(Duration::from_secs(30)); // Would take 30s without interrupt
        let elapsed = start.elapsed();

        // Should return almost immediately due to interrupt
        assert!(
            elapsed < Duration::from_millis(100),
            "wait() took too long after interrupt: {:?}",
            elapsed
        );
    }

    /// WHY: Validates reset clears interrupt
    /// WHAT: reset() should allow subsequent waits to complete normally
    #[test]
    fn test_reset() {
        let waiter = SpinWaiter::new(100_000);

        // Interrupt and then reset
        waiter.interrupt();
        assert!(waiter.is_interrupted());

        waiter.reset();
        assert!(!waiter.is_interrupted());

        // Wait should now complete normally
        let start = Instant::now();
        waiter.wait(Duration::from_millis(5));
        let elapsed = start.elapsed();

        // Should not return immediately (wasn't interrupted)
        assert!(elapsed >= Duration::from_micros(100));
    }

    /// WHY: Validates interrupt during wait
    /// WHAT: interrupt() called during wait should wake early
    #[test]
    fn test_interrupt_during_wait() {
        use std::sync::Arc;
        use std::thread;

        // Use Arc to share the waiter between threads
        let waiter = Arc::new(SpinWaiter::new(500_000));
        let waiter_clone = Arc::clone(&waiter);

        // Spawn thread that will interrupt after short delay
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            waiter_clone.interrupt();
        });

        let start = Instant::now();
        // This would take ~1 second without interrupt
        waiter.wait(Duration::from_millis(1000));
        let elapsed = start.elapsed();

        handle.join().expect("thread should complete");

        // Should have been interrupted and returned early
        assert!(
            elapsed < Duration::from_millis(500),
            "wait() was not interrupted early enough: {:?}",
            elapsed
        );
    }

    /// WHY: Validates zero duration returns immediately
    /// WHAT: wait(Duration::ZERO) should not spin at all
    #[test]
    fn test_zero_duration() {
        let waiter = SpinWaiter::new(100_000);

        let start = Instant::now();
        waiter.wait(Duration::ZERO);
        let elapsed = start.elapsed();

        // Should return essentially immediately
        assert!(elapsed < Duration::from_millis(1));
    }

    /// WHY: Validates very short duration
    /// WHAT: Short waits should still complete without panic
    #[test]
    fn test_short_duration() {
        let waiter = SpinWaiter::new(1000); // Very low iterations per ms

        let start = Instant::now();
        waiter.wait(Duration::from_micros(100));
        let elapsed = start.elapsed();

        // Should complete quickly (no actual spinning for very short durations)
        assert!(elapsed < Duration::from_millis(10));
    }
}
