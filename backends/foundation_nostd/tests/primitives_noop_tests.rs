use foundation_nostd::primitives::noop::*;
use foundation_nostd::primitives::TryLockError;

    // NoopMutex tests
    #[test]
    fn test_mutex_new_and_into_inner() {
        let mutex = NoopMutex::new(42);
        assert_eq!(mutex.into_inner(), 42);
    }

    #[test]
    fn test_mutex_lock() {
        let mutex = NoopMutex::new(0);
        {
            let mut guard = mutex.lock().unwrap();
            *guard += 1;
            assert_eq!(*guard, 1);
        }
        let guard = mutex.lock().unwrap();
        assert_eq!(*guard, 1);
    }

    #[test]
    fn test_mutex_try_lock_success() {
        let mutex = NoopMutex::new(42);
        let guard = mutex.try_lock();
        assert!(guard.is_ok());
        assert_eq!(*guard.unwrap(), 42);
    }

    #[test]
    fn test_mutex_try_lock_would_block() {
        let mutex = NoopMutex::new(42);
        let _guard1 = mutex.lock().unwrap();
        let result = mutex.try_lock();
        assert!(matches!(result, Err(TryLockError::WouldBlock)));
    }

    #[test]
    fn test_mutex_is_locked() {
        let mutex = NoopMutex::new(0);
        assert!(!mutex.is_locked());
        let _guard = mutex.lock().unwrap();
        assert!(mutex.is_locked());
    }

    #[test]
    fn test_mutex_get_mut() {
        let mut mutex = NoopMutex::new(0);
        *mutex.get_mut().unwrap() = 42;
        assert_eq!(*mutex.lock().unwrap(), 42);
    }

    // NoopRwLock tests
    #[test]
    fn test_rwlock_new_and_into_inner() {
        let lock = NoopRwLock::new(42);
        assert_eq!(lock.into_inner(), 42);
    }

    #[test]
    fn test_rwlock_read() {
        let lock = NoopRwLock::new(42);
        let r = lock.read().unwrap();
        assert_eq!(*r, 42);
    }

    #[test]
    fn test_rwlock_multiple_readers() {
        let lock = NoopRwLock::new(42);
        let r1 = lock.read().unwrap();
        let r2 = lock.read().unwrap();
        let r3 = lock.read().unwrap();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
        assert_eq!(*r3, 42);
    }

    #[test]
    fn test_rwlock_write() {
        let lock = NoopRwLock::new(0);
        {
            let mut w = lock.write().unwrap();
            *w += 1;
            assert_eq!(*w, 1);
        }
        let r = lock.read().unwrap();
        assert_eq!(*r, 1);
    }

    #[test]
    fn test_rwlock_try_read_success() {
        let lock = NoopRwLock::new(42);
        let r = lock.try_read();
        assert!(r.is_ok());
        assert_eq!(*r.unwrap(), 42);
    }

    #[test]
    fn test_rwlock_try_write_success() {
        let lock = NoopRwLock::new(42);
        let w = lock.try_write();
        assert!(w.is_ok());
    }

    #[test]
    fn test_rwlock_get_mut() {
        let mut lock = NoopRwLock::new(0);
        *lock.get_mut() = 42;
        assert_eq!(*lock.read().unwrap(), 42);
    }

    // NoopOnce tests
    #[test]
    fn test_once_new() {
        let once = NoopOnce::new();
        assert!(!once.is_completed());
    }

    #[test]
    fn test_once_call_once() {
        let once = NoopOnce::new();
        let mut called = 0;

        once.call_once(|| {
            called += 1;
        });
        assert_eq!(called, 1);
        assert!(once.is_completed());

        once.call_once(|| {
            called += 1;
        });
        assert_eq!(called, 1); // Should not be called again
    }

    #[test]
    fn test_once_debug() {
        let once = NoopOnce::new();
        let debug_str = format!("{once:?}");
        assert!(debug_str.contains("NoopOnce"));
    }
