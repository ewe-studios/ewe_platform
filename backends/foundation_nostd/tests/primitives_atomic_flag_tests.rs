use foundation_nostd::primitives::atomic_flag::*;
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// WHY: Validates `AtomicFlag` construction and initial state
    /// WHAT: Creating a flag should set initial value correctly
    #[test]
    fn test_new() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.is_set());

        let flag = AtomicFlag::new(true);
        assert!(flag.is_set());
    }

    /// WHY: Validates set operation
    /// WHAT: `set()` should change flag to true
    #[test]
    fn test_set() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.is_set());

        flag.set();
        assert!(flag.is_set());

        // Setting again should be idempotent
        flag.set();
        assert!(flag.is_set());
    }

    /// WHY: Validates clear operation
    /// WHAT: `clear()` should change flag to false
    #[test]
    fn test_clear() {
        let flag = AtomicFlag::new(true);
        assert!(flag.is_set());

        flag.clear();
        assert!(!flag.is_set());

        // Clearing again should be idempotent
        flag.clear();
        assert!(!flag.is_set());
    }

    /// WHY: Validates `test_and_set` atomic operation
    /// WHAT: Should return old value and set to true
    #[test]
    fn test_test_and_set() {
        let flag = AtomicFlag::new(false);
        assert!(!flag.test_and_set());
        assert!(flag.is_set());

        // Second call should return true
        assert!(flag.test_and_set());
        assert!(flag.is_set());
    }

    /// WHY: Validates `test_and_clear` atomic operation
    /// WHAT: Should return old value and set to false
    #[test]
    fn test_test_and_clear() {
        let flag = AtomicFlag::new(true);
        assert!(flag.test_and_clear());
        assert!(!flag.is_set());

        // Second call should return false
        assert!(!flag.test_and_clear());
        assert!(!flag.is_set());
    }

    /// WHY: Validates `compare_and_swap` success case
    /// WHAT: CAS should succeed when current value matches
    #[test]
    fn test_compare_and_swap_success() {
        let flag = AtomicFlag::new(false);

        assert_eq!(flag.compare_and_swap(false, true), Ok(false));
        assert!(flag.is_set());

        assert_eq!(flag.compare_and_swap(true, false), Ok(true));
        assert!(!flag.is_set());
    }

    /// WHY: Validates `compare_and_swap` failure case
    /// WHAT: CAS should fail when current value doesn't match
    #[test]
    fn test_compare_and_swap_failure() {
        let flag = AtomicFlag::new(false);

        assert_eq!(flag.compare_and_swap(true, false), Err(false));
        assert!(!flag.is_set());
    }

    /// WHY: Validates relaxed load operation
    /// WHAT: `load_relaxed` should return current value
    #[test]
    fn test_load_relaxed() {
        let flag = AtomicFlag::new(true);
        assert!(flag.load_relaxed());

        flag.clear();
        assert!(!flag.load_relaxed());
    }

    /// WHY: Validates relaxed store operation
    /// WHAT: `store_relaxed` should set value
    #[test]
    fn test_store_relaxed() {
        let flag = AtomicFlag::new(false);

        flag.store_relaxed(true);
        assert!(flag.load_relaxed());

        flag.store_relaxed(false);
        assert!(!flag.load_relaxed());
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should create flag initialized to false
    #[test]
    fn test_default() {
        let flag = AtomicFlag::default();
        assert!(!flag.is_set());
    }

    /// WHY: Validates Debug implementation
    /// WHAT: Debug formatting should show current value
    #[test]
    fn test_debug() {
        let flag = AtomicFlag::new(true);
        let debug_str = format!("{flag:?}");
        assert!(debug_str.contains("AtomicFlag"));
        assert!(debug_str.contains("true"));
    }

    /// WHY: Validates From<bool> implementation
    /// WHAT: From trait should create flag from bool
    #[test]
    fn test_from_bool() {
        let flag = AtomicFlag::from(true);
        assert!(flag.is_set());

        let flag = AtomicFlag::from(false);
        assert!(!flag.is_set());
    }
