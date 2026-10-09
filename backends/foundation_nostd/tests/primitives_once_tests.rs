use foundation_nostd::primitives::once::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// `WHY`: Validates basic once initialization
    /// `WHAT`: Function should execute exactly once
    #[test]
    fn test_call_once() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let once = Once::new();

        once.call_once(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        once.call_once(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
    }

    /// `WHY`: Validates `is_completed` returns false initially
    /// `WHAT`: New Once should not be completed
    #[test]
    fn test_not_completed_initially() {
        let once = Once::new();
        assert!(!once.is_completed());
    }

    /// `WHY`: Validates `is_completed` returns true after `call_once`
    /// `WHAT`: Once should be completed after initialization
    #[test]
    fn test_completed_after_call() {
        let once = Once::new();
        once.call_once(|| {});
        assert!(once.is_completed());
    }

    /// `WHY`: Validates `call_once`_force with non-poisoned state
    /// `WHAT`: Should execute and report not poisoned
    #[test]
    fn test_call_once_force() {
        let once = Once::new();
        let mut executed = false;

        once.call_once_force(|state| {
            assert!(!state.is_poisoned());
            executed = true;
        });

        assert!(executed);
        assert!(once.is_completed());
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create incomplete Once
    #[test]
    fn test_default() {
        let once = Once::default();
        assert!(!once.is_completed());
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: Once should be Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Once>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: Once should be Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Once>();
    }
