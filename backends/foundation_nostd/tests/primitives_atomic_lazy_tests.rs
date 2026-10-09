use foundation_nostd::primitives::atomic_lazy::*;
    #[cfg(not(feature = "std"))]
    use alloc::{format, vec};
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// `WHY`: Validates lazy initialization on first access
    /// `WHAT`: Initializer should run exactly once
    #[test]
    fn test_lazy_init() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let lazy = AtomicLazy::new(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
            42
        });

        assert_eq!(*lazy, 42);
        assert_eq!(*lazy, 42); // Should not reinitialize
        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
    }

    /// `WHY`: Validates force method
    /// `WHAT`: Force should initialize and return reference
    #[test]
    fn test_force() {
        let lazy = AtomicLazy::new(|| 42);
        let value = AtomicLazy::force(&lazy);
        assert_eq!(*value, 42);
    }

    /// `WHY`: Validates Deref implementation
    /// `WHAT`: Deref should trigger initialization
    #[test]
    fn test_deref() {
        let lazy = AtomicLazy::new(|| vec![1, 2, 3]);
        assert_eq!(lazy.len(), 3);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create lazy with default value
    #[test]
    fn test_default() {
        let lazy = AtomicLazy::<i32>::default();
        assert_eq!(*lazy, 0);
    }

    /// `WHY`: Validates Debug implementation
    /// `WHAT`: Debug should show the value
    #[test]
    fn test_debug() {
        let lazy = AtomicLazy::new(|| 42);
        let debug = format!("{lazy:?}");
        assert!(debug.contains("42") || debug.contains("AtomicLazy"));
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: `AtomicLazy` should be Send when T and F are Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AtomicLazy<i32>>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: `AtomicLazy` should be Sync when T: Sync and F: Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AtomicLazy<i32>>();
    }
