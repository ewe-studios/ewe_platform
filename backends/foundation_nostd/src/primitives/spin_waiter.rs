//! Iteration-based waiter for single-threaded and WASM environments.
//!
//! Provides duration-based waiting using spin-loop iteration counting instead of
//! OS sleep mechanisms. This is necessary for WASM and `no_std` environments where
//! `std::thread::sleep` is not available.
//!
//! # Platform-specific behavior
//!
//! - **WASM**: Uses ~100,000 iterations per millisecond (adjustable)
//! - **`no_std` embedded**: Uses ~1,000,000 iterations per millisecond (adjustable)
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
    /// Clone creates a new `SpinWaiter` with the same configuration.
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
    /// Creates a new `SpinWaiter` with platform-specific defaults.
    ///
    /// # Arguments
    ///
    /// * `iterations_per_ms` - Estimated spin-loop iterations per millisecond
    ///   - WASM: ~100,000 iterations/ms
    ///   - `no_std` embedded: ~1,000,000 iterations/ms
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

    /// Creates a new `SpinWaiter` for WASM environments.
    ///
    /// Uses 100,000 iterations per millisecond as a reasonable default for WASM.
    #[inline]
    #[must_use]
    pub const fn wasm() -> Self {
        Self::new(100_000)
    }

    /// Creates a new `SpinWaiter` for embedded `no_std` environments.
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
        // Check for interrupt every 1000 iterations
        const INTERRUPT_CHECK_INTERVAL: u64 = 1000;

        // Calculate total iterations needed (avoid u128 cast)
        let total_spins = dur.as_secs() * 1000 * self.iterations_per_ms
            + u64::from(dur.subsec_millis()) * self.iterations_per_ms;

        // Cap at maximum to prevent infinite spinning
        let spins_to_do = total_spins.min(self.max_total_iterations);

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
