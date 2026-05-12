pub trait Drain {
    fn drain(&mut self);
}

impl<T, I> Drain for T
where
    T: Iterator<Item = I>,
{
    fn drain(&mut self) {
        for item in self {
            drop(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test that Drain works with non-Clone iterators.
    /// This verifies the blanket impl doesn't require Clone bound.
    #[test]
    fn test_drain_with_non_clone_iterator() {
        // VecDeque's iterator is not Clone
        let items: std::collections::VecDeque<i32> =
            vec![1, 2, 3].into_iter().collect();

        // This would fail to compile if Drain required Clone
        let mut iter = items.into_iter();
        iter.drain();

        // Iterator should be exhausted
        assert!(iter.next().is_none());
    }

    /// Test that Drain properly drops all items.
    #[test]
    fn test_drain_drops_all_items() {
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct DropCounter;

        impl Drop for DropCounter {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        // Reset counter
        DROP_COUNT.store(0, Ordering::SeqCst);

        let items: Vec<DropCounter> = (0..5).map(|_| DropCounter).collect();
        let mut iter = items.into_iter();
        iter.drain();

        // All 5 DropCounters should have been dropped
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 5);
    }
}
