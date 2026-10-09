use foundation_nostd::spin_mutex::*;
use foundation_nostd::primitives::TryLockError;

    /// `WHY`: Validates basic mutex construction and `into_inner`
    /// `WHAT`: Creating a mutex and extracting its value should work
    #[test]
    fn test_new_and_into_inner() {
        let mutex = SpinMutex::new(42);
        assert_eq!(mutex.into_inner().unwrap(), 42);
    }

    /// `WHY`: Validates basic `lock` acquisition and data access
    /// `WHAT`: Lock should be acquirable and data should be accessible
    #[test]
    fn test_lock() {
        let mutex = SpinMutex::new(0);
        {
            let mut guard = mutex.lock().unwrap();
            *guard += 1;
            assert_eq!(*guard, 1);
        }
        let guard = mutex.lock().unwrap();
        assert_eq!(*guard, 1);
    }

    /// `WHY`: Validates `try_lock` behavior when `lock` is free
    /// `WHAT`: `try_lock` should succeed when `lock` is not held
    #[test]
    fn test_try_lock_success() {
        let mutex = SpinMutex::new(42);
        let guard = mutex.try_lock();
        assert!(guard.is_ok());
        assert_eq!(*guard.unwrap(), 42);
    }

    /// `WHY`: Validates `try_lock` behavior when `lock` is held
    /// `WHAT`: `try_lock` should return `WouldBlock` when already held
    #[test]
    fn test_try_lock_would_block() {
        let mutex = SpinMutex::new(42);
        let _guard1 = mutex.lock().unwrap();
        let result = mutex.try_lock();
        assert!(matches!(result, Err(TryLockError::WouldBlock)));
    }

    /// `WHY`: Validates that mutex is not poisoned by default
    /// `WHAT`: New mutex should not be poisoned
    #[test]
    fn test_not_poisoned() {
        let mutex = SpinMutex::new(42);
        assert!(!mutex.is_poisoned());
    }

    /// `WHY`: Validates `get_mut` functionality
    /// `WHAT`: `get_mut` should provide mutable access without locking
    #[test]
    fn test_get_mut() {
        let mut mutex = SpinMutex::new(0);
        *mutex.get_mut().unwrap() = 42;
        assert_eq!(*mutex.lock().unwrap(), 42);
    }

    /// `WHY`: Validates Send trait bounds
    /// `WHAT`: Mutex should be Send when T is Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SpinMutex<i32>>();
    }

    /// `WHY`: Validates Sync trait bounds
    /// `WHAT`: Mutex should be Sync when T is Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SpinMutex<i32>>();
    }
