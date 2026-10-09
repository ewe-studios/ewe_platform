use foundation_nostd::primitives::once_lock::*;

    /// `WHY`: Validates basic `OnceLock` creation and get
    /// `WHAT`: New `OnceLock` should return None
    #[test]
    fn test_new_and_get() {
        let lock = OnceLock::<i32>::new();
        assert!(lock.get().is_none());
    }

    /// `WHY`: Validates set and get work together
    /// `WHAT`: After set, get should return the value
    #[test]
    fn test_set_and_get() {
        let lock = OnceLock::new();
        assert!(lock.set(42).is_ok());
        assert_eq!(*lock.get().unwrap(), 42);
    }

    /// `WHY`: Validates set only succeeds once
    /// `WHAT`: Second set should return error
    #[test]
    fn test_set_twice() {
        let lock = OnceLock::new();
        assert!(lock.set(42).is_ok());
        match lock.set(43) {
            Err(_) => {} // Expected
            Ok(()) => panic!("Second set should fail"),
        }
    }

    /// `WHY`: Validates `get_or_init` lazy initialization
    /// `WHAT`: Should initialize on first call
    #[test]
    fn test_get_or_init() {
        let lock = OnceLock::new();
        let value = lock.get_or_init(|| 42);
        assert_eq!(*value, 42);

        let value2 = lock.get_or_init(|| 43);
        assert_eq!(*value2, 42); // Still the first value
    }

    /// `WHY`: Validates `get_or_try_init` with success
    /// `WHAT`: Should initialize on first call with Ok result
    #[test]
    fn test_get_or_try_init_ok() {
        let lock = OnceLock::new();
        let value = lock.get_or_try_init(|| Ok::<_, ()>(42)).unwrap();
        assert_eq!(*value, 42);
    }

    /// `WHY`: Validates take removes the value
    /// `WHAT`: After take, get should return None
    #[test]
    fn test_take() {
        let mut lock = OnceLock::new();
        lock.set(42).unwrap();
        assert_eq!(lock.take(), Some(42));
        assert!(lock.get().is_none());
    }

    /// `WHY`: Validates `into_inner` consumes and returns value
    /// `WHAT`: Should return Some(value) if set
    #[test]
    fn test_into_inner() {
        let lock = OnceLock::new();
        lock.set(42).unwrap();
        assert_eq!(lock.into_inner(), Some(42));
    }

    /// `WHY`: Validates From implementation
    /// `WHAT`: From should create initialized `OnceLock`
    #[test]
    fn test_from() {
        let lock = OnceLock::from(42);
        assert_eq!(*lock.get().unwrap(), 42);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create empty `OnceLock`
    #[test]
    fn test_default() {
        let lock = OnceLock::<i32>::default();
        assert!(lock.get().is_none());
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: `OnceLock` should be Send when T: Send + Sync
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OnceLock<i32>>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: `OnceLock` should be Sync when T: Send + Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<OnceLock<i32>>();
    }
