//! Tests for `io::mem::memory`: `MemoryLimiter`, `Arena`, `TypeArena` and
//! `ArenaPool`.

mod memory_limiter_tests {
    use foundation_core::io::mem::memory::{MemoryErrors, MemoryLimiter};

    #[test]
    fn cant_expand_usage_pass_limits() {
        let limiter_rc = MemoryLimiter::create_shared(10);
        let mut limiter = limiter_rc.borrow_mut();

        assert_eq!(limiter.current_usage(), 0);

        assert!(matches!(
            limiter.increase_usage(15),
            Err(MemoryErrors::MemoryLimitExceededError)
        ));
        assert_eq!(limiter.current_usage(), 15);
    }

    #[test]
    fn can_get_current_usage() {
        let limiter_rc = MemoryLimiter::create_shared(10);
        let mut limiter = limiter_rc.borrow_mut();

        assert_eq!(limiter.current_usage(), 0);

        assert!(matches!(limiter.increase_usage(5), Ok(())));
        assert_eq!(limiter.current_usage(), 5);
    }
}

mod arena_tests {
    use std::rc::Rc;

    use foundation_core::io::mem::memory::{Arena, MemoryErrors, MemoryLimiter};

    #[test]
    fn can_create_arena() {
        let limiter = MemoryLimiter::create_shared(10);
        let mut arena = Arena::new(Rc::clone(&limiter), 2);

        arena.append(&[1, 2]).unwrap();
        assert_eq!(arena.bytes(), &[1, 2]);
        assert_eq!(limiter.borrow().current_usage(), 2);

        arena.append(&[3, 4]).unwrap();
        assert_eq!(arena.bytes(), &[1, 2, 3, 4]);
        assert_eq!(limiter.borrow().current_usage(), 4);

        arena.append(&[5, 6, 7, 8, 9, 10]).unwrap();
        assert_eq!(arena.bytes(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(limiter.borrow().current_usage(), 10);

        let err = arena.append(&[11]).unwrap_err();

        assert_eq!(err, MemoryErrors::MemoryLimitExceededError);
    }
}

mod type_area_tests {
    use std::rc::Rc;

    use foundation_core::io::mem::memory::{MemoryErrors, MemoryLimiter, TypeArena};

    #[test]
    fn current_usage() {
        {
            let limiter = MemoryLimiter::create_shared(10);
            let mut vec_u8: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

            vec_u8.push(1).unwrap();
            vec_u8.push(2).unwrap();
            assert_eq!(limiter.borrow().current_usage(), 2);
        }

        {
            let limiter = MemoryLimiter::create_shared(10);
            let mut vec_u32: TypeArena<u32> = TypeArena::new(Rc::clone(&limiter));

            vec_u32.push(1).unwrap();
            vec_u32.push(2).unwrap();
            assert_eq!(limiter.borrow().current_usage(), 8);
        }
    }

    #[test]
    fn max_limit() {
        let limiter = MemoryLimiter::create_shared(2);
        let mut vector: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

        vector.push(1).unwrap();
        vector.push(2).unwrap();

        let err = vector.push(3).unwrap_err();

        assert_eq!(err, MemoryErrors::MemoryLimitExceededError);
    }

    #[test]
    fn drop() {
        let limiter = MemoryLimiter::create_shared(1);

        {
            let mut vector: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

            vector.push(1).unwrap();
            assert_eq!(limiter.borrow().current_usage(), 1);
        }

        assert_eq!(limiter.borrow().current_usage(), 0);
    }

    #[test]
    fn drain() {
        let limiter = MemoryLimiter::create_shared(10);
        let mut vector: TypeArena<u8> = TypeArena::new(Rc::clone(&limiter));

        vector.push(1).unwrap();
        vector.push(2).unwrap();
        vector.push(3).unwrap();
        assert_eq!(limiter.borrow().current_usage(), 3);

        vector.drain(0..3);
        assert_eq!(limiter.borrow().current_usage(), 0);

        vector.push(1).unwrap();
        vector.push(2).unwrap();
        vector.push(3).unwrap();
        vector.push(4).unwrap();
        assert_eq!(limiter.borrow().current_usage(), 4);

        vector.drain(1..=2);
        assert_eq!(limiter.borrow().current_usage(), 2);
    }
}

mod arena_pool_tests {
    use foundation_core::io::mem::memory::{ArenaPool, FnGenerator, MemoryLimiter, Resettable};

    /// A pointer-sized resettable value. The inline version implemented
    /// `Resettable` for `&'static u8` directly, which the orphan rule forbids
    /// outside the crate; this newtype keeps the same 8-byte size, so the
    /// allocation accounting asserted below is unchanged.
    struct ResettableU8(&'static u8);

    impl Resettable for ResettableU8 {
        fn reset(&mut self) {
            self.0 = &0;
        }
    }

    #[test]
    fn test_arena_pool() {
        let limiter = MemoryLimiter::create_shared(800);
        let mut pool: ArenaPool<ResettableU8> =
            ArenaPool::new(&limiter, FnGenerator::new(|_| ResettableU8(&0)));

        assert_eq!(pool.allocated(), 0);

        let mut my_number = pool.allocate().unwrap();

        assert_eq!(pool.allocated(), 8);

        my_number.0 = &1;

        assert_eq!(*my_number.0, 1);

        pool.deallocate(my_number);
    }

    #[test]
    fn test_arena_pool_limits() {
        let limiter = MemoryLimiter::create_shared(8);
        let mut pool: ArenaPool<ResettableU8> =
            ArenaPool::new(&limiter, FnGenerator::new(|_| ResettableU8(&0)));

        assert_eq!(pool.allocated(), 0);

        _ = pool.allocate().unwrap();

        assert_eq!(pool.allocated(), 8);

        assert!(pool.allocate().is_err());
    }
}
