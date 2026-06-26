use foundation_nostd::primitives::raw_once::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// `WHY`: Validates basic once initialization
    /// `WHAT`: Function should execute exactly once
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

    /// `WHY`: Validates `is_completed` returns false initially
    /// `WHAT`: New `RawOnce` should not be completed
    #[test]
    fn test_not_completed_initially() {
        let once = RawOnce::new();
        assert!(!once.is_completed());
    }

    /// `WHY`: Validates `is_completed` returns true after `call_once`
    /// `WHAT`: `RawOnce` should be completed after initialization
    #[test]
    fn test_completed_after_call() {
        let once = RawOnce::new();
        once.call_once(|| {});
        assert!(once.is_completed());
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create incomplete `RawOnce`
    #[test]
    fn test_default() {
        let once = RawOnce::default();
        assert!(!once.is_completed());
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: `RawOnce` should be Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RawOnce>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: `RawOnce` should be Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RawOnce>();
    }
