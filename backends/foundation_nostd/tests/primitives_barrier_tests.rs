use foundation_nostd::primitives::barrier::*;
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// WHY: Validates barrier construction
    /// WHAT: Creating a barrier with count should work
    #[test]
    fn test_new() {
        let barrier = SpinBarrier::new(3);
        assert_eq!(barrier.num_threads, 3);
    }

    /// WHY: Validates barrier panics on zero count
    /// WHAT: Creating barrier with 0 threads should panic
    #[test]
    #[should_panic(expected = "barrier count must be > 0")]
    fn test_new_zero_panics() {
        let _ = SpinBarrier::new(0);
    }

    /// WHY: Validates single thread barrier
    /// WHAT: Barrier with n=1 should immediately return leader
    #[test]
    fn test_single_thread() {
        let barrier = SpinBarrier::new(1);
        let result = barrier.wait();
        assert!(result.is_leader());
    }

    /// WHY: Validates barrier is reusable
    /// WHAT: Barrier should work for multiple rounds
    #[test]
    fn test_reusable() {
        let barrier = SpinBarrier::new(1);

        // First round
        let result1 = barrier.wait();
        assert!(result1.is_leader());

        // Second round
        let result2 = barrier.wait();
        assert!(result2.is_leader());
    }

    /// WHY: Validates Debug implementation
    /// WHAT: Debug formatting should work
    #[test]
    fn test_debug() {
        let barrier = SpinBarrier::new(3);
        let debug = format!("{barrier:?}");
        assert!(debug.contains("SpinBarrier"));
        assert!(debug.contains("num_threads"));
    }
