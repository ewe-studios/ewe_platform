use foundation_nostd::primitives::raw_spin_rwlock::*;

    /// `WHY`: Validates basic construction and `into_inner`
    /// `WHAT`: Creating `lock` and extracting value should work
    #[test]
    fn test_new_and_into_inner() {
        let lock = RawSpinRwLock::new(42);
        assert_eq!(lock.into_inner(), 42);
    }

    /// `WHY`: Validates `read` `lock` acquisition
    /// `WHAT`: Read `lock` should be acquirable
    #[test]
    fn test_read() {
        let lock = RawSpinRwLock::new(42);
        let guard = lock.read();
        assert_eq!(*guard, 42);
    }

    /// `WHY`: Validates `write` `lock` acquisition
    /// `WHAT`: Write `lock` should be acquirable and allow mutation
    #[test]
    fn test_write() {
        let lock = RawSpinRwLock::new(0);
        {
            let mut guard = lock.write();
            *guard = 42;
        }
        assert_eq!(*lock.read(), 42);
    }

    /// `WHY`: Validates multiple simultaneous readers
    /// `WHAT`: Multiple `read` guards should coexist
    #[test]
    fn test_multiple_readers() {
        let lock = RawSpinRwLock::new(42);
        let r1 = lock.read();
        let r2 = lock.read();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
    }

    /// `WHY`: Validates `try_read` when `lock` is free
    /// `WHAT`: `try_read` should succeed when no writers
    #[test]
    fn test_try_read_success() {
        let lock = RawSpinRwLock::new(42);
        let guard = lock.try_read();
        assert!(guard.is_some());
    }

    /// `WHY`: Validates `try_write` when `lock` is free
    /// `WHAT`: `try_write` should succeed when unlocked
    #[test]
    fn test_try_write_success() {
        let lock = RawSpinRwLock::new(42);
        let guard = lock.try_write();
        assert!(guard.is_some());
    }

    /// `WHY`: Validates Send trait bounds
    /// `WHAT`: `RwLock` should be Send when T is Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RawSpinRwLock<i32>>();
    }

    /// `WHY`: Validates Sync trait bounds
    /// `WHAT`: `RwLock` should be Sync when T is Send + Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RawSpinRwLock<i32>>();
    }
