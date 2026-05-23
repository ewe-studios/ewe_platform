//! Cooperative spin waiter for external timer-based resumption.
//!
//! WHY: On wasm32, `thread::sleep` is unavailable and busy-spinning blocks the
//! JS event loop. The waiter tells an external timer (setTimeout, host FFI) to
//! wake it up after a duration, returning `Waiting` immediately — no spinning.
//!
//! WHAT: Generic cooperative waiter — no JS awareness. The caller provides the
//! `schedule_resume` callback that integrates with their timer subsystem.
//!
//! HOW: `wait(dur)` calls `schedule_resume(dur, resume_closure)` and returns
//! `WaitStatus::Waiting` immediately. The resume_closure sets an `AtomicBool`
//! flag when the timer fires. Callers check `has_resumed()` to know if the
//! wait period has elapsed.

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(feature = "std")]
use std::sync::Arc;

use core::sync::atomic::{AtomicBool, Ordering};

/// Status returned by [`CooperativeSpinWaiter::wait()`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitStatus {
    /// The runtime has been told to schedule a resume.
    /// Caller should stop processing and yield.
    Waiting,
}

/// Signature for the schedule_resume callback.
/// The consumer provides the external timer mechanism (setTimeout, host FFI, etc.).
pub type ScheduleResumeFn = fn(core::time::Duration, Box<dyn FnOnce()>);

/// Cooperative spin waiter that tells the runtime "wake me up after X"
/// and returns [`WaitStatus::Waiting`] immediately — no spinning.
pub struct CooperativeSpinWaiter {
    resumed: Arc<AtomicBool>,
    iterations_per_ms: u64,
    schedule_resume: ScheduleResumeFn,
}

impl Clone for CooperativeSpinWaiter {
    fn clone(&self) -> Self {
        Self {
            resumed: Arc::clone(&self.resumed),
            iterations_per_ms: self.iterations_per_ms,
            schedule_resume: self.schedule_resume,
        }
    }
}

impl CooperativeSpinWaiter {
    /// Creates a new waiter.
    ///
    /// # Arguments
    ///
    /// * `iterations_per_ms` — spin-loop iterations per ms (for fallback)
    /// * `schedule_resume` — external timer scheduling function
    #[inline]
    #[must_use]
    pub fn new(iterations_per_ms: u64, schedule_resume: ScheduleResumeFn) -> Self {
        Self {
            resumed: Arc::new(AtomicBool::new(false)),
            iterations_per_ms,
            schedule_resume,
        }
    }

    /// Tell the runtime to wake me up after `dur`.
    /// Returns [`WaitStatus::Waiting`] immediately — no spinning.
    #[inline]
    pub fn wait(&self, dur: core::time::Duration) -> WaitStatus {
        let resumed = Arc::clone(&self.resumed);
        (self.schedule_resume)(dur, Box::new(move || {
            resumed.store(true, Ordering::Release);
        }));
        WaitStatus::Waiting
    }

    /// Check if the timer has fired since the last `wait()` or `reset()`.
    #[inline]
    #[must_use]
    pub fn has_resumed(&self) -> bool {
        self.resumed.load(Ordering::Acquire)
    }

    /// Clear the resumed flag for reuse.
    #[inline]
    pub fn reset(&self) {
        self.resumed.store(false, Ordering::Release);
    }

    /// Get the configured iterations per millisecond.
    #[inline]
    #[must_use]
    pub const fn iterations_per_ms(&self) -> u64 {
        self.iterations_per_ms
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use core::time::Duration;
    use std::sync::Mutex;

    thread_local! {
        static TEST_RESUME_HOLDER: Mutex<Option<Box<dyn FnOnce()>>> = const { Mutex::new(None) };
    }

    fn test_schedule_fn(dur: Duration, resume: Box<dyn FnOnce()>) {
        let _ = dur;
        TEST_RESUME_HOLDER.with(|h| *h.lock().unwrap() = Some(resume));
    }

    /// WHY: Validate that wait() returns Waiting immediately without spinning.
    #[test]
    fn test_wait_returns_waiting_immediately() {
        fn no_op_schedule(_dur: Duration, _resume: Box<dyn FnOnce()>) {}
        let waiter = CooperativeSpinWaiter::new(100_000, no_op_schedule);
        let result = waiter.wait(Duration::from_millis(10));
        assert_eq!(result, WaitStatus::Waiting);
    }

    /// WHY: Validate that calling the resume closure sets has_resumed() to true.
    #[test]
    fn test_resume_sets_flag() {
        let waiter = CooperativeSpinWaiter::new(100_000, test_schedule_fn);
        assert!(!waiter.has_resumed());

        waiter.wait(Duration::from_millis(10));

        // Trigger the stored resume closure
        TEST_RESUME_HOLDER.with(|h| {
            if let Some(resume) = h.lock().unwrap().take() {
                resume();
            }
        });

        assert!(waiter.has_resumed());
    }

    /// WHY: Validate that reset() clears the resumed flag.
    #[test]
    fn test_reset_clears_flag() {
        let waiter = CooperativeSpinWaiter::new(100_000, test_schedule_fn);

        waiter.wait(Duration::from_millis(10));

        TEST_RESUME_HOLDER.with(|h| {
            if let Some(resume) = h.lock().unwrap().take() {
                resume();
            }
        });

        assert!(waiter.has_resumed());

        waiter.reset();

        assert!(!waiter.has_resumed());
    }

    /// WHY: Validate iterations_per_ms is stored correctly.
    #[test]
    fn test_iterations_per_ms() {
        fn no_op_schedule(_dur: Duration, _resume: Box<dyn FnOnce()>) {}
        let waiter = CooperativeSpinWaiter::new(50_000, no_op_schedule);
        assert_eq!(waiter.iterations_per_ms(), 50_000);
    }
}
