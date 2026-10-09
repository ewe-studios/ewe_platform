//! Once-initialization primitive with poisoning support.
//!
//! This matches the `std::sync::Once` API for `no_std` environments.

use core::sync::atomic::{AtomicU8, Ordering};

// States for initialization
const INCOMPLETE: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;
const POISONED: u8 = 3;

/// A synchronization primitive for one-time initialization with poisoning.
///
/// If the initialization closure panics, the `Once` becomes poisoned.
pub struct Once {
    state: AtomicU8,
}

/// State returned by `Once::call_once_force`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnceState {
    poisoned: bool,
}

impl OnceState {
    /// Returns `true` if the `Once` was poisoned.
    #[inline]
    #[must_use]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

impl Once {
    /// Creates a new incomplete `Once`.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(INCOMPLETE),
        }
    }

    /// Executes the closure if this is the first call.
    ///
    /// If the closure panics, the `Once` becomes poisoned and subsequent
    /// calls will panic immediately.
    ///
    /// # Examples
    ///
    /// # Panics
    ///
    /// Panics if the `Once` instance is poisoned from a previous panic.
    ///
    /// ```
    /// use foundation_nostd::primitives::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// INIT.call_once(|| {
    ///     // Initialization code
    /// });
    /// ```
    pub fn call_once<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        struct PoisonOnPanic<'a>(&'a Once);

        impl Drop for PoisonOnPanic<'_> {
            fn drop(&mut self) {
                self.0.state.store(POISONED, Ordering::Release);
            }
        }

        let state = self.state.load(Ordering::Acquire);

        if state == COMPLETE {
            return;
        }

        assert!(state != POISONED, "Once instance has been poisoned");

        // Try to transition from INCOMPLETE to RUNNING
        if self
            .state
            .compare_exchange(INCOMPLETE, RUNNING, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // We won the race, execute the function

            let guard = PoisonOnPanic(self);
            f();
            core::mem::forget(guard);
            self.state.store(COMPLETE, Ordering::Release);
        } else {
            // Someone else is running or has completed
            loop {
                let state = self.state.load(Ordering::Acquire);
                match state {
                    COMPLETE => return,
                    POISONED => panic!("Once instance has been poisoned"),
                    _ => core::hint::spin_loop(),
                }
            }
        }
    }

    /// Executes the closure even if the `Once` is poisoned.
    ///
    /// The closure receives an `OnceState` indicating if the `Once` was poisoned.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::primitives::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// INIT.call_once_force(|state| {
    ///     if state.is_poisoned() {
    ///         // Recovery code
    ///     }
    /// });
    /// ```
    pub fn call_once_force<F>(&self, f: F)
    where
        F: FnOnce(OnceState),
    {
        struct ResetOnPanic<'a>(&'a Once);

        impl Drop for ResetOnPanic<'_> {
            fn drop(&mut self) {
                self.0.state.store(POISONED, Ordering::Release);
            }
        }

        let mut state = self.state.load(Ordering::Acquire);

        if state == COMPLETE {
            return;
        }

        let poisoned = state == POISONED;

        // Try to transition to RUNNING
        loop {
            match self
                .state
                .compare_exchange(state, RUNNING, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(s) => {
                    if s == COMPLETE {
                        return;
                    }
                    state = s;
                    core::hint::spin_loop();
                }
            }
        }

        // Execute the function

        let guard = ResetOnPanic(self);
        f(OnceState { poisoned });
        core::mem::forget(guard);
        self.state.store(COMPLETE, Ordering::Release);
    }

    /// Returns `true` if initialization has completed successfully.
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }
}

impl Default for Once {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
