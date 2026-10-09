use foundation_nostd::primitives::raw_spin_mutex::*;

    /// `WHY`: Validates basic mutex construction and `into_inner`
    /// `WHAT`: Creating a mutex and extracting its value should work
    #[test]
    fn test_new_and_into_inner() {
        let mutex = RawSpinMutex::new(42);
        assert_eq!(mutex.into_inner(), 42);
    }

    /// `WHY`: Validates basic `lock` acquisition and data access
    /// `WHAT`: Lock should be acquirable and data should be accessible
    #[test]
    fn test_lock() {
        let mutex = RawSpinMutex::new(0);
        {
            let mut guard = mutex.lock();
            *guard += 1;
            assert_eq!(*guard, 1);
        }
        let guard = mutex.lock();
        assert_eq!(*guard, 1);
    }

    /// `WHY`: Validates `try_lock` behavior when `lock` is free
    /// `WHAT`: `try_lock` should succeed when `lock` is not held
    #[test]
    fn test_try_lock_success() {
        let mutex = RawSpinMutex::new(42);
        let guard = mutex.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.unwrap(), 42);
    }

    /// `WHY`: Validates `try_lock` behavior when `lock` is held
    /// `WHAT`: `try_lock` should fail when `lock` is already held
    #[test]
    fn test_try_lock_failure() {
        let mutex = RawSpinMutex::new(42);
        let _guard1 = mutex.lock();
        let guard2 = mutex.try_lock();
        assert!(guard2.is_none());
    }

    /// `WHY`: Validates `try_lock_with_spin_limit` functionality
    /// `WHAT`: Should succeed within limit when `lock` becomes available
    #[test]
    fn test_try_lock_with_spin_limit() {
        let mutex = RawSpinMutex::new(42);

        // Should succeed immediately when lock is free
        let guard = mutex.try_lock_with_spin_limit(1000);
        assert!(guard.is_some());
        drop(guard);

        // Should fail within limit when lock is held
        let _guard1 = mutex.lock();
        let guard2 = mutex.try_lock_with_spin_limit(10);
        assert!(guard2.is_none());
    }

    /// `WHY`: Validates `get_mut` functionality with exclusive borrow
    /// `WHAT`: `get_mut` should provide mutable access without locking
    #[test]
    fn test_get_mut() {
        let mut mutex = RawSpinMutex::new(0);
        *mutex.get_mut() = 42;
        assert_eq!(*mutex.lock(), 42);
    }

    /// `WHY`: Validates Send trait bounds
    /// `WHAT`: Mutex should be Send when T is Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RawSpinMutex<i32>>();
    }

    /// `WHY`: Validates Sync trait bounds
    /// `WHAT`: Mutex should be Sync when T is Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RawSpinMutex<i32>>();
    }
