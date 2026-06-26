use foundation_nostd::primitives::reader_spin_rwlock::*;
    extern crate std;
    use std::vec;

    #[test]
    fn test_new() {
        let lock = ReaderSpinRwLock::new(42);
        assert_eq!(*lock.read().unwrap(), 42);
    }

    #[test]
    fn test_read() {
        let lock = ReaderSpinRwLock::new(vec![1, 2, 3]);
        let r1 = lock.read().unwrap();
        let r2 = lock.read().unwrap();
        assert_eq!(*r1, vec![1, 2, 3]);
        assert_eq!(*r2, vec![1, 2, 3]);
    }

    #[test]
    fn test_write() {
        let lock = ReaderSpinRwLock::new(0);
        {
            let mut w = lock.write().unwrap();
            *w = 42;
        }
        assert_eq!(*lock.read().unwrap(), 42);
    }

    #[test]
    fn test_try_read() {
        let lock = ReaderSpinRwLock::new(42);
        assert!(lock.try_read().is_ok());
    }

    #[test]
    fn test_try_write() {
        let lock = ReaderSpinRwLock::new(42);
        assert!(lock.try_write().is_ok());
    }

    #[test]
    fn test_reader_preferring_no_block_on_writer_waiting() {
        // This test demonstrates reader-preferring behavior:
        // Readers can acquire even while writer is trying to acquire
        let lock = ReaderSpinRwLock::new(0);

        let _r1 = lock.read().unwrap();
        // Writer would set WRITER_WAITING in writer-preferring lock
        // But reader-preferring has no such flag
        let r2 = lock.read(); // Should succeed

        assert!(r2.is_ok());
    }

    #[test]
    fn test_into_inner() {
        let lock = ReaderSpinRwLock::new(42);
        assert_eq!(lock.into_inner().unwrap(), 42);
    }

    #[test]
    fn test_get_mut() {
        let mut lock = ReaderSpinRwLock::new(42);
        *lock.get_mut().unwrap() = 100;
        assert_eq!(*lock.read().unwrap(), 100);
    }

    // Note: Poisoning test disabled in no_std environment.
    // In no_std, poisoning must be triggered manually or through
    // external panic runtime, as we cannot detect panics automatically.
