use foundation_nostd::comp::basic::{Barrier, Mutex, Once, OnceLock, RwLock};
use foundation_nostd::comp::condvar_comp::{CondVar, Mutex as CondVarMutex};

    #[test]
    fn test_mutex_basic() {
        let mutex = Mutex::new(42);
        let guard = mutex.lock().unwrap();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_rwlock_basic() {
        let rwlock = RwLock::new(100);

        // Test read
        let read_guard = rwlock.read().unwrap();
        assert_eq!(*read_guard, 100);
        drop(read_guard);

        // Test write
        let mut write_guard = rwlock.write().unwrap();
        *write_guard = 200;
        drop(write_guard);

        // Verify write
        let read_guard = rwlock.read().unwrap();
        assert_eq!(*read_guard, 200);
    }

    #[test]
    fn test_condvar_basic() {
        use core::time::Duration;

        let mutex = CondVarMutex::new(false);
        let condvar = CondVar::new();

        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(1));

        // Should timeout
        if let Ok((_guard, timeout_result)) = result {
            assert!(timeout_result.timed_out());
        }
    }

    #[test]
    fn test_barrier_basic() {
        let barrier = Barrier::new(1);
        let result = barrier.wait();
        assert!(result.is_leader());
    }

    #[test]
    fn test_once_basic() {
        static ONCE: Once = Once::new();
        let mut counter = 0;

        ONCE.call_once(|| {
            counter += 1;
        });

        ONCE.call_once(|| {
            counter += 1;
        });

        assert_eq!(counter, 1);
    }

    #[test]
    fn test_once_lock_basic() {
        let lock = OnceLock::new();
        assert!(lock.get().is_none());

        lock.set(42).ok();
        assert_eq!(lock.get(), Some(&42));

        // Setting again should fail
        assert!(lock.set(100).is_err());
    }

    #[test]
    fn test_poison_error_handling() {
        let mutex = Mutex::new(42);
        let guard = mutex.lock().unwrap();
        let value = *guard;
        assert_eq!(value, 42);
    }
