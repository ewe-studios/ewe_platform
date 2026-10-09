use foundation_nostd::primitives::atomic_cell::*;

    /// `WHY`: Validates basic atomic cell creation and load
    /// `WHAT`: New cell should contain initial value
    #[test]
    fn test_new_and_load() {
        let cell = AtomicCell::new(42);
        assert_eq!(cell.load(), 42);
    }

    /// `WHY`: Validates store updates the value
    /// `WHAT`: After store, load should return new value
    #[test]
    fn test_store() {
        let cell = AtomicCell::new(0);
        cell.store(42);
        assert_eq!(cell.load(), 42);
    }

    /// `WHY`: Validates swap returns old value
    /// `WHAT`: Swap should return previous value and update cell
    #[test]
    fn test_swap() {
        let cell = AtomicCell::new(1);
        let old = cell.swap(2);
        assert_eq!(old, 1);
        assert_eq!(cell.load(), 2);
    }

    /// `WHY`: Validates `compare_exchange` on success
    /// `WHAT`: Should swap when current matches
    #[test]
    fn test_compare_exchange_success() {
        let cell = AtomicCell::new(1);
        let result = cell.compare_exchange(1, 2);
        assert_eq!(result, Ok(1));
        assert_eq!(cell.load(), 2);
    }

    /// `WHY`: Validates `compare_exchange` on failure
    /// `WHAT`: Should not swap when current doesn't match
    #[test]
    fn test_compare_exchange_failure() {
        let cell = AtomicCell::new(1);
        let result = cell.compare_exchange(5, 2);
        assert_eq!(result, Err(1));
        assert_eq!(cell.load(), 1);
    }

    /// `WHY`: Validates `get_mut` provides mutable access
    /// `WHAT`: `get_mut` should allow direct mutation
    #[test]
    fn test_get_mut() {
        let mut cell = AtomicCell::new(0);
        *cell.get_mut() = 10;
        assert_eq!(cell.load(), 10);
    }

    /// `WHY`: Validates `into_inner` consumes and returns value
    /// `WHAT`: Should return the inner value
    #[test]
    fn test_into_inner() {
        let cell = AtomicCell::new(42);
        assert_eq!(cell.into_inner(), 42);
    }

    /// `WHY`: Validates Default implementation
    /// `WHAT`: Default should create cell with default value
    #[test]
    fn test_default() {
        let cell = AtomicCell::<i32>::default();
        assert_eq!(cell.load(), 0);
    }

    /// `WHY`: Validates From implementation
    /// `WHAT`: From should create cell from value
    #[test]
    fn test_from() {
        let cell = AtomicCell::from(42);
        assert_eq!(cell.load(), 42);
    }

    /// `WHY`: Validates Send bound requirement
    /// `WHAT`: `AtomicCell` should be Send when T: Send
    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AtomicCell<i32>>();
    }

    /// `WHY`: Validates Sync bound requirement
    /// `WHAT`: `AtomicCell` should be Sync when T: Send
    #[test]
    fn test_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<AtomicCell<i32>>();
    }
