use foundation_nostd::primitives::spin_rwlock::*;

    #[cfg(not(feature = "std"))]
    use alloc::format;
    #[cfg(not(feature = "std"))]
    use alloc::vec;

    /// `WHY`: Validates basic `lock` construction and `into_inner`
    /// `WHAT`: Creating a `lock` and extracting its value should work
    #[test]
    fn test_new_and_into_inner() {
        let lock = SpinRwLock::new(42);
        assert!(!lock.is_poisoned());
        assert_eq!(lock.into_inner().unwrap(), 42);
    }

    /// `WHY`: Validates basic `read` `lock` acquisition
    /// `WHAT`: Read locks should be acquirable and data accessible
    #[test]
    fn test_read() {
        let lock = SpinRwLock::new(vec![1, 2, 3]);
        let r = lock.read().unwrap();
        assert_eq!(*r, vec![1, 2, 3]);
    }

    /// `WHY`: Validates multiple simultaneous readers
    /// `WHAT`: Multiple `read` guards should coexist
    #[test]
    fn test_multiple_readers() {
        let lock = SpinRwLock::new(42);
        let r1 = lock.read().unwrap();
        let r2 = lock.read().unwrap();
        let r3 = lock.read().unwrap();
        assert_eq!(*r1, 42);
        assert_eq!(*r2, 42);
        assert_eq!(*r3, 42);
    }

    /// `WHY`: Validates `write` `lock` acquisition and mutation
    /// `WHAT`: Write `lock` should provide exclusive mutable access
    #[test]
    fn test_write() {
        let lock = SpinRwLock::new(0);
        {
            let mut w = lock.write().unwrap();
            *w += 1;
            assert_eq!(*w, 1);
        }
        let r = lock.read().unwrap();
        assert_eq!(*r, 1);
    }

    /// `WHY`: Validates `write` exclusivity
    /// `WHAT`: `try_read` should fail when `write` `lock` is held
    #[test]
    fn test_write_blocks_readers() {
        let lock = SpinRwLock::new(42);
        let _w = lock.write().unwrap();
        assert!(lock.try_read().is_err());
    }

    /// `WHY`: Validates reader blocking during `write`
    /// `WHAT`: `try_write` should fail when `read` `lock` is held
    #[test]
    fn test_readers_block_writers() {
        let lock = SpinRwLock::new(42);
        let _r = lock.read().unwrap();
        assert!(lock.try_write().is_err());
    }

    /// `WHY`: Validates `try_read` when `lock` is free
    /// `WHAT`: `try_read` should succeed when no writers
    #[test]
    fn test_try_read_success() {
        let lock = SpinRwLock::new(42);
        let r = lock.try_read();
        assert!(r.is_ok());
        assert_eq!(*r.unwrap(), 42);
    }

    /// `WHY`: Validates `try_write` when `lock` is free
    /// `WHAT`: `try_write` should succeed when no readers or writers
    #[test]
    fn test_try_write_success() {
        let lock = SpinRwLock::new(42);
        let w = lock.try_write();
        assert!(w.is_ok());
    }

    /// `WHY`: Validates `get_mut` functionality
    /// `WHAT`: `get_mut` should provide mutable access without locking
    #[test]
    fn test_get_mut() {
        let mut lock = SpinRwLock::new(0);
        *lock.get_mut().unwrap() = 42;
        assert_eq!(*lock.read().unwrap(), 42);
    }

    /// `WHY`: Validates `is_poisoned` detection
    /// `WHAT`: Fresh `lock` should not be poisoned
    #[test]
    fn test_not_poisoned() {
        let lock = SpinRwLock::new(0);
        assert!(!lock.is_poisoned());
    }

    /// `WHY`: Validates Send trait bounds
    /// `WHAT`: `RwLock` should be Send when T is Send + Sync
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SpinRwLock<i32>>();
    }

    /// `WHY`: Validates Sync trait bounds
    /// `WHAT`: `RwLock` should be Sync when T is Send + Sync
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SpinRwLock<i32>>();
    }

    /// `WHY`: Validates Debug implementation
    /// `WHAT`: Debug formatting should work for both locked and unlocked states
    #[test]
    fn test_debug() {
        let lock = SpinRwLock::new(42);
        let debug_str = format!("{lock:?}");
        assert!(debug_str.contains("SpinRwLock"));
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create `lock` with default value
    #[test]
    fn test_default() {
        let lock = SpinRwLock::<i32>::default();
        assert_eq!(*lock.read().unwrap(), 0);
    }

    /// `WHY`: Validates From<T> implementation
    /// `WHAT`: From trait should allow creating `lock` from value
    #[test]
    fn test_from() {
        let lock = SpinRwLock::from(42);
        assert_eq!(*lock.read().unwrap(), 42);
    }
