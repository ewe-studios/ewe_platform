use foundation_nostd::primitives::poison::*;

    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// `WHY`: Validates `PoisonError` construction and `into_inner` recovery
    /// `WHAT`: Creating a `PoisonError` should allow extracting the wrapped value
    #[test]
    fn test_poison_error_into_inner() {
        let guard = 42;
        let error = PoisonError::new(guard);
        assert_eq!(error.into_inner(), 42);
    }

    /// `WHY`: Validates `PoisonError` reference access methods
    /// `WHAT`: `get_ref` and `get_mut` should provide access to the wrapped guard
    #[test]
    fn test_poison_error_get_ref() {
        let mut error = PoisonError::new(42);
        assert_eq!(*error.get_ref(), 42);
        *error.get_mut() = 100;
        assert_eq!(*error.get_ref(), 100);
    }

    /// `WHY`: Validates `TryLockError` enum variants and conversions
    /// `WHAT`: `TryLockError` should support both `WouldBlock` and Poisoned variants
    #[test]
    fn test_try_lock_error_variants() {
        let would_block: TryLockError<i32> = TryLockError::WouldBlock;
        assert!(matches!(would_block, TryLockError::WouldBlock));

        let poison = PoisonError::new(42);
        let poisoned: TryLockError<i32> = TryLockError::from(poison);
        assert!(matches!(poisoned, TryLockError::Poisoned(_)));
    }

    /// `WHY`: Validates error messages for user-facing display
    /// `WHAT`: Display trait should provide meaningful error messages
    #[test]
    fn test_error_display() {
        let poison = PoisonError::new(42);
        let msg = format!("{poison}");
        assert!(msg.contains("poisoned"));

        let would_block: TryLockError<i32> = TryLockError::WouldBlock;
        let msg = format!("{would_block}");
        assert!(msg.contains("already held") || msg.contains("try_lock failed"));
    }
