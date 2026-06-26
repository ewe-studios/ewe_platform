//! Raw once-initialization primitive without poisoning support.
//!
//! This provides simple one-time initialization for `no_std` environments
//! where poisoning is unnecessary (e.g., panic = abort).

use core::sync::atomic::{AtomicU8, Ordering};

// States for initialization
const INCOMPLETE: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;

/// A synchronization primitive for one-time initialization.
///
/// This is simpler than `Once` and doesn't track poisoning.
pub struct RawOnce {
    state: AtomicU8,
}

impl RawOnce {
    /// Creates a new incomplete `RawOnce`.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(INCOMPLETE),
        }
    }

    /// Executes the closure if this is the first call.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::RawOnce;
    ///
    /// static INIT: `RawOnce` = RawOnce::new();
    ///
    /// INIT.call_once(|| {
    ///     // Initialization code
    /// });
    /// ```
    pub fn call_once<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        if self.is_completed() {
            return;
        }

        // Try to transition from INCOMPLETE to RUNNING
        if self
            .state
            .compare_exchange(INCOMPLETE, RUNNING, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // We won the race, execute the function
            f();
            self.state.store(COMPLETE, Ordering::Release);
        } else {
            // Someone else is running or has completed
            // Spin until complete
            while !self.is_completed() {
                core::hint::spin_loop();
            }
        }
    }

    /// Returns `true` if initialization has completed.
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }
}

impl Default for RawOnce {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
