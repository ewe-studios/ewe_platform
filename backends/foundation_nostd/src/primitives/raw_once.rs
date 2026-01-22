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
    /// static INIT: RawOnce = RawOnce::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// WHY: Validates basic once initialization
    /// WHAT: Function should execute exactly once
    #[test]
    fn test_call_once() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let once = RawOnce::new();

        once.call_once(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        once.call_once(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
    }

    /// WHY: Validates is_completed returns false initially
    /// WHAT: New RawOnce should not be completed
    #[test]
    fn test_not_completed_initially() {
        let once = RawOnce::new();
        assert!(!once.is_completed());
    }

    /// WHY: Validates is_completed returns true after call_once
    /// WHAT: RawOnce should be completed after initialization
    #[test]
    fn test_completed_after_call() {
        let once = RawOnce::new();
        once.call_once(|| {});
        assert!(once.is_completed());
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should create incomplete RawOnce
    #[test]
    fn test_default() {
        let once = RawOnce::default();
        assert!(!once.is_completed());
    }

    /// WHY: Validates Send bound requirement
    /// WHAT: RawOnce should be Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RawOnce>();
    }

    /// WHY: Validates Sync bound requirement
    /// WHAT: RawOnce should be Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RawOnce>();
    }
}
