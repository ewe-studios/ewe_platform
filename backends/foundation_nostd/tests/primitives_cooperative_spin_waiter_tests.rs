extern crate std;

use foundation_nostd::primitives::cooperative_spin_waiter::*;
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
