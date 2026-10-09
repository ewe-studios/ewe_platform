use foundation_nostd::primitives::spin_wait::*;

    /// `WHY`: Validates `SpinWait` construction
    /// `WHAT`: Creating a `SpinWait` should start with `counter` at 0
    #[test]
    fn test_new() {
        let spin = SpinWait::new();
        assert_eq!(spin.counter(), 0);
        assert!(!spin.is_exhausted());
    }

    /// `WHY`: Validates `spin` progression
    /// `WHAT`: Each `spin()` call should increment `counter` until exhausted
    #[test]
    fn test_spin_progression() {
        let mut spin = SpinWait::new();
        let mut count = 0;

        while spin.spin() {
            count += 1;
        }

        assert!(count > 0);
        assert!(spin.is_exhausted());
    }

    /// `WHY`: Validates `spin` returns false when exhausted
    /// `WHAT`: `spin()` should return false after `SPIN_LIMIT` iterations
    #[test]
    fn test_spin_exhaustion() {
        let mut spin = SpinWait::new();

        for _ in 0..SPIN_LIMIT {
            assert!(spin.spin());
        }

        assert!(!spin.spin());
        assert!(spin.is_exhausted());
    }

    /// `WHY`: Validates `reset` functionality
    /// `WHAT`: `reset()` should allow restarting `spin` sequence
    #[test]
    fn test_reset() {
        let mut spin = SpinWait::new();

        while spin.spin() {}
        assert!(spin.is_exhausted());

        spin.reset();
        assert_eq!(spin.counter(), 0);
        assert!(!spin.is_exhausted());
        assert!(spin.spin());
    }

    /// `WHY`: Validates `spin_once` doesn't affect `counter`
    /// `WHAT`: `spin_once()` should `spin` without incrementing `counter`
    #[test]
    fn test_spin_once() {
        let spin = SpinWait::new();
        assert_eq!(spin.counter(), 0);

        spin.spin_once();
        assert_eq!(spin.counter(), 0);

        spin.spin_once();
        assert_eq!(spin.counter(), 0);
    }

    /// `WHY`: Validates `counter` tracking
    /// `WHAT`: `counter()` should return number of successful spins
    #[test]
    fn test_counter() {
        let mut spin = SpinWait::new();
        assert_eq!(spin.counter(), 0);

        spin.spin();
        assert_eq!(spin.counter(), 1);

        spin.spin();
        assert_eq!(spin.counter(), 2);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create fresh `SpinWait`
    #[test]
    fn test_default() {
        let spin = SpinWait::default();
        assert_eq!(spin.counter(), 0);
        assert!(!spin.is_exhausted());
    }
