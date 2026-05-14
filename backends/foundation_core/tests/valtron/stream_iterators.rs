// Integration tests for StreamIterator combinators
use foundation_core::valtron::{
    CollectAll, MapAllDone, MapAllPendingAndDone, Stream, StreamIteratorExt,
};

// Simple test stream iterator for unit tests
struct TestStream {
    items: Vec<Stream<u32, String>>,
}

impl TestStream {
    fn new(items: Vec<Stream<u32, String>>) -> Self {
        Self { items }
    }
}

impl Iterator for TestStream {
    type Item = Stream<u32, String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.pop()
    }
}

// Simple stream iterator wrapper for Vec<Stream<D, P>>
struct SimpleStream<D, P> {
    items: std::vec::IntoIter<Stream<D, P>>,
}

impl<D, P> SimpleStream<D, P> {
    fn from_vec(vec: Vec<D>) -> Self {
        let items = vec
            .into_iter()
            .map(Stream::Next)
            .collect::<Vec<_>>()
            .into_iter();
        Self { items }
    }
}

impl<D, P> Iterator for SimpleStream<D, P> {
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.next()
    }
}

#[test]
fn test_map_done() {
    let items = vec![
        Stream::Pending("wait".to_string()),
        Stream::Next(10),
        Stream::Next(5),
    ];
    let stream = TestStream::new(items);
    let mut mapped = stream.map_done(|x| x * 2);

    // pop() returns items in reverse order: 5, 10, Pending
    // After mapping: 10 (5*2), 20 (10*2), Pending
    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(10)));
    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(20)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::Pending("wait".to_string()))
    );
}

#[test]
fn test_map_pending() {
    let items = vec![Stream::Pending("wait".to_string()), Stream::Next(5)];
    let stream = TestStream::new(items);
    let mut mapped = stream.map_pending(|s: String| s.len());

    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(5)));
    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Pending(4)));
}

#[test]
fn test_filter_done() {
    let items = vec![Stream::Next(3), Stream::Next(10), Stream::Next(5)];
    let stream = TestStream::new(items);
    let mut filtered = stream.filter_done(|x| *x > 5);

    // pop() returns: Next(5), Next(10), Next(3)
    // filter: 5 > 5 = false → Ignore, 10 > 5 = true → Next(10), 3 > 5 = false → Ignore
    assert_eq!(Iterator::next(&mut filtered), Some(Stream::Ignore)); // 5 was filtered out
    assert_eq!(Iterator::next(&mut filtered), Some(Stream::Next(10)));
    assert_eq!(Iterator::next(&mut filtered), Some(Stream::Ignore)); // 3 was filtered out
    assert_eq!(Iterator::next(&mut filtered), None);
}

#[test]
fn test_collect() {
    let items = vec![
        Stream::Next(3),
        Stream::Next(2),
        Stream::Pending("wait".to_string()),
        Stream::Next(1),
    ];
    let stream = TestStream::new(items);
    let mut collected = StreamIteratorExt::collect(stream);

    // pop() returns: Next(1), Pending, Next(2), Next(3)
    // First Next(1) is collected, returns Ignore
    assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

    // Pending passes through unchanged
    assert_eq!(
        Iterator::next(&mut collected),
        Some(Stream::Pending("wait".to_string()))
    );

    // Next(2) is collected, returns Ignore
    assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

    // Next(3) is collected, returns Ignore
    assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

    // Should yield collected Vec at the end
    match Iterator::next(&mut collected) {
        Some(Stream::Next(vec)) => {
            assert_eq!(vec.len(), 3);
            assert!(vec.contains(&1));
            assert!(vec.contains(&2));
            assert!(vec.contains(&3));
        }
        other => panic!("Expected Next(Vec), got {other:?}"),
    }

    // Should be done after yielding the collected result
    assert_eq!(Iterator::next(&mut collected), None);
}

#[test]
fn test_map_delayed() {
    use std::time::Duration;

    let items = vec![Stream::Delayed(Duration::from_secs(1)), Stream::Next(5)];
    let stream = TestStream::new(items);
    let mut mapped = stream.map_delayed(|d| d * 2);

    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(5)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::Delayed(Duration::from_secs(2)))
    );
}

#[test]
fn test_split_collector_map_sends_transformed_items() {
    let items = vec![
        Stream::Pending("wait".to_string()),
        Stream::Next(10),
        Stream::Next(3),
        Stream::Next(20),
    ];
    let stream = TestStream::new(items);

    // Observer gets string representations of Next items > 5
    let (observer, mut continuation) = stream.split_collector_map::<_, String, ()>(
        |item| match item {
            Stream::Next(v) if *v > 5 => (true, Some(Stream::Next(format!("val:{v}")))),
            _ => (false, None),
        },
        10,
    );

    // Drive the continuation to completion
    while Iterator::next(&mut continuation).is_some() {}

    let collected: Vec<_> = observer
        .filter_map(|item| match item {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .collect();

    assert_eq!(collected.len(), 2);
    assert!(collected.contains(&"val:20".to_string()));
    assert!(collected.contains(&"val:10".to_string()));
}

#[test]
fn test_split_collector_map_transform_returns_none_skips() {
    let items = vec![Stream::Next(10), Stream::Next(5)];
    let stream = TestStream::new(items);

    // Matched but transform returns None for odd numbers → skipped
    let (observer, mut continuation) = stream.split_collector_map::<_, u64, ()>(
        |item| match item {
            Stream::Next(v) if v % 2 == 0 => (true, Some(Stream::Next(u64::from(*v)))),
            Stream::Next(_) => (true, None),
            _ => (false, None),
        },
        10,
    );

    while Iterator::next(&mut continuation).is_some() {}

    let collected: Vec<_> = observer
        .filter_map(|item| match item {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .collect();

    assert_eq!(collected, vec![10u64]);
}

#[test]
fn test_split_collect_one_map() {
    let items = vec![Stream::Next(3), Stream::Next(10), Stream::Next(20)];
    let stream = TestStream::new(items);

    let (observer, mut continuation) =
        stream.split_collect_one_map::<_, String, ()>(|item| match item {
            Stream::Next(v) if *v > 5 => (true, Some(Stream::Next(v.to_string()))),
            _ => (false, None),
        });

    while Iterator::next(&mut continuation).is_some() {}

    let collected: Vec<_> = observer
        .filter_map(|item| match item {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .collect();

    // Queue size 1, so only first match gets through
    assert_eq!(collected.len(), 1);
}

#[test]
fn test_collect_all() {
    // Create two test streams
    let stream1 = TestStream::new(vec![Stream::Next(1u32), Stream::Next(2)]);
    let stream2 = TestStream::new(vec![Stream::Next(10u32), Stream::Next(20)]);

    let mut combined = CollectAll::new(vec![stream1, stream2]);

    // Should collect all values and yield single Next with combined results
    let mut collected_values = Vec::new();
    for item in &mut combined {
        if let Stream::Next(values) = item {
            collected_values.extend(values);
        }
    }

    assert_eq!(collected_values.len(), 4);
    assert!(collected_values.contains(&1));
    assert!(collected_values.contains(&2));
    assert!(collected_values.contains(&10));
    assert!(collected_values.contains(&20));
}

#[test]
fn test_map_all_done() {
    // Create two test streams
    let stream1 = TestStream::new(vec![Stream::Next(1u32)]);
    let stream2 = TestStream::new(vec![Stream::Next(10u32)]);

    let mut mapper = MapAllDone::new(vec![stream1, stream2], |values: Vec<u32>| {
        values.iter().sum::<u32>()
    });

    // Should apply mapper when all sources produce values
    let mut got_result = false;
    for item in &mut mapper {
        if let Stream::Next(sum) = item {
            assert_eq!(sum, 11); // 1 + 10
            got_result = true;
        }
    }

    assert!(got_result, "Should have produced mapped result");
}

#[test]
fn test_map_all_pending_and_done() {
    // Create two test streams
    let stream1 = TestStream::new(vec![Stream::Next(1u32)]);
    let stream2 = TestStream::new(vec![Stream::Next(10u32)]);

    let mut mapper = MapAllPendingAndDone::new(
        vec![stream1, stream2],
        |states: Vec<Stream<u32, String>>| states.len(),
    );

    // Should apply mapper to all states
    let mut got_result = false;
    for item in &mut mapper {
        if let Stream::Next(count) = item {
            assert_eq!(count, 2); // Two states
            got_result = true;
        }
    }

    assert!(got_result, "Should have produced mapped result");
}

#[test]
fn test_map_iter_flattens_nested_iterators() {
    // Test using the extension method with a simple test stream
    struct VecStream {
        items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
    }

    impl Iterator for VecStream {
        type Item = Stream<Vec<u32>, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Next(vec![1u32, 2u32]),
        Stream::Next(vec![3u32, 4u32, 5u32]),
    ];
    let stream = VecStream {
        items: items.into_iter(),
    };

    let mut flattened = stream.map_iter(|item: Stream<Vec<u32>, String>| {
        let vec = match item {
            Stream::Next(v) => v,
            _ => vec![],
        };
        SimpleStream::<u32, String>::from_vec(vec)
    });

    // First inner setup returns Ignore, then 1, 2
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    )); // Setting up first inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));
    // Second inner setup returns Ignore, then 3, 4, 5
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    )); // Setting up second inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(3))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(4))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(5))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_map_iter_passes_through_pending() {
    // Test using the extension method with a simple test stream
    struct VecStream {
        items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
    }

    impl Iterator for VecStream {
        type Item = Stream<Vec<u32>, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Next(vec![1u32]),
        Stream::Pending("wait".to_string()),
        Stream::Next(vec![2u32, 3u32]),
    ];
    let stream = VecStream {
        items: items.into_iter(),
    };

    let mut flattened = stream.map_iter(|item: Stream<Vec<u32>, String>| {
        let vec = match item {
            Stream::Next(v) => v,
            // For non-Next items, return empty stream (they are consumed)
            _ => vec![],
        };
        SimpleStream::<u32, String>::from_vec(vec)
    });

    // Ignore when setting up first inner, then 1
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    // Pending consumed by mapper (returns empty vec) = Ignore
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    // Empty inner exhausted, setup next inner from Next(vec![2,3]) = Ignore
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    // Then 2, 3 from second inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(3))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_map_iter_different_pending_types() {
    // Mapper receives full Stream and can decide what to do with non-Next states
    struct VecStream {
        items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
    }

    impl Iterator for VecStream {
        type Item = Stream<Vec<u32>, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Next(vec![1u32, 2u32]),
        Stream::Pending("outer_pending".to_string()),
        Stream::Next(vec![3u32]),
    ];
    let stream = VecStream {
        items: items.into_iter(),
    };

    let mut flattened = stream.map_iter(|item: Stream<Vec<u32>, String>| {
        let vec = match item {
            Stream::Next(v) => v,
            _ => vec![],
        };
        SimpleStream::<u32, String>::from_vec(vec)
    });

    // Ignore for setup, then 1, 2 from first inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));
    // Outer's Pending consumed by mapper (returns empty stream) - setup = Ignore
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    // Then setup next inner = Ignore, then 3
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(3))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_flatten_next() {
    struct VecStream {
        items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
    }

    impl Iterator for VecStream {
        type Item = Stream<Vec<u32>, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Next(vec![1u32, 2u32]),
        Stream::Pending("wait".to_string()),
        Stream::Next(vec![3u32, 4u32, 5u32]),
    ];
    let stream = VecStream {
        items: items.into_iter(),
    };

    let mut flattened = stream.flatten_next();

    // First inner yields 1, 2 (with Ignore when setting up)
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    )); // Setting up first inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));
    // Pending passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(_))
    ));
    // Setting up second inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(3))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(4))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(5))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_flat_map_next() {
    struct NumStream {
        items: std::vec::IntoIter<Stream<u32, String>>,
    }

    impl Iterator for NumStream {
        type Item = Stream<u32, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Next(2u32),
        Stream::Next(3u32),
        Stream::Pending("wait".to_string()),
    ];
    let stream = NumStream {
        items: items.into_iter(),
    };

    // Map each number to a range [0, n), then flatten
    let mut flattened = stream.flat_map_next(|n| 0..n);

    // 2 -> [0, 1]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    )); // Setting up inner for 2
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    // 3 -> [0, 1, 2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    )); // Setting up inner for 3
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));
    // Pending passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(_))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_flatten_pending() {
    struct VecStream {
        items: std::vec::IntoIter<Stream<u32, Vec<String>>>,
    }

    impl Iterator for VecStream {
        type Item = Stream<u32, Vec<String>>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Pending(vec!["a".to_string(), "b".to_string()]),
        Stream::Next(42u32),
        Stream::Pending(vec!["c".to_string()]),
    ];
    let stream = VecStream {
        items: items.into_iter(),
    };

    let mut flattened = stream.flatten_pending();

    // First inner yields "a", "b"
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    )); // Setting up first inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(ref s)) if s == "a"
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(ref s)) if s == "b"
    ));
    // Next passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(42))
    ));
    // Setting up second inner
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(ref s)) if s == "c"
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_flat_map_pending() {
    struct NumStream {
        items: std::vec::IntoIter<Stream<u32, u32>>,
    }

    impl Iterator for NumStream {
        type Item = Stream<u32, u32>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        Stream::Pending(2u32),
        Stream::Next(99u32),
        Stream::Pending(3u32),
    ];
    let stream = NumStream {
        items: items.into_iter(),
    };

    // Map each pending number to a range [0, n), then flatten
    let mut flattened = stream.flat_map_pending(|n| 0..n);

    // 2 -> [0, 1]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(1))
    ));
    // Next passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(99))
    ));
    // 3 -> [0, 1, 2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(2))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// ConcurrentQueueStreamIterator Tests
// ============================================================================

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::ConcurrentQueueStreamIterator;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_concurrent_queue_stream_iterator_returns_items() {
    let queue: Arc<ConcurrentQueue<Stream<u32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));

    // Push some items to the queue
    queue.push(Stream::Next(1u32)).unwrap();
    queue.push(Stream::Next(2u32)).unwrap();
    queue.push(Stream::Next(3u32)).unwrap();

    let mut iter = ConcurrentQueueStreamIterator::new(queue.clone(), 10, Duration::from_nanos(20));

    // Should receive all items
    assert!(matches!(iter.next(), Some(Stream::Next(1))));
    assert!(matches!(iter.next(), Some(Stream::Next(2))));
    assert!(matches!(iter.next(), Some(Stream::Next(3))));
}

#[test]
fn test_concurrent_queue_stream_iterator_yields_ignore_after_max_turns() {
    let queue: Arc<ConcurrentQueue<Stream<u32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));
    // Queue is empty

    let mut iter = ConcurrentQueueStreamIterator::new(queue.clone(), 3, Duration::from_nanos(1));

    // Should yield Ignore after max_turns (3) unsuccessful polls
    let mut ignore_count = 0;
    for _ in 0..5 {
        if let Some(Stream::Ignore) = iter.next() {
            ignore_count += 1
        }
    }

    // Should have yielded Ignore multiple times
    assert!(
        ignore_count > 0,
        "Iterator should yield Ignore after max_turns"
    );
}

#[test]
fn test_concurrent_queue_stream_iterator_returns_none_when_closed() {
    let queue: Arc<ConcurrentQueue<Stream<u32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));

    // Push an item then close the queue
    queue.push(Stream::Next(1u32)).unwrap();
    queue.close();

    let mut iter = ConcurrentQueueStreamIterator::new(queue.clone(), 10, Duration::from_nanos(20));

    // Should receive the item
    assert!(matches!(iter.next(), Some(Stream::Next(1))));

    // Then should return None (queue closed)
    assert_eq!(iter.next(), None);
}

#[test]
#[should_panic]
fn test_concurrent_queue_stream_iterator_panics_on_zero_max_turns() {
    let queue: Arc<ConcurrentQueue<Stream<u32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));
    let _ = ConcurrentQueueStreamIterator::new(queue.clone(), 0, Duration::from_nanos(20));
}

#[test]
fn test_concurrent_queue_stream_iterator_accessors() {
    let queue: Arc<ConcurrentQueue<Stream<u32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));
    queue.push(Stream::Next(1u32)).unwrap();
    queue.push(Stream::Next(2u32)).unwrap();

    let iter = ConcurrentQueueStreamIterator::new(queue.clone(), 5, Duration::from_nanos(50));

    // Test accessor methods
    assert_eq!(iter.max_turns(), 5);
    assert_eq!(iter.park_duration(), Duration::from_nanos(50));
    assert_eq!(iter.len(), 2);
    assert!(!iter.is_empty());
    assert!(!iter.is_closed());
}

#[test]
fn test_concurrent_queue_stream_iterator_passes_through_stream_variants() {
    let queue: Arc<ConcurrentQueue<Stream<u32, String>>> = Arc::new(ConcurrentQueue::bounded(10));

    // Push different Stream variants
    queue.push(Stream::Init).unwrap();
    queue.push(Stream::Ignore).unwrap();
    queue.push(Stream::Pending("loading".to_string())).unwrap();
    queue
        .push(Stream::Delayed(Duration::from_millis(100)))
        .unwrap();
    queue.push(Stream::Next(42u32)).unwrap();

    let mut iter = ConcurrentQueueStreamIterator::new(queue.clone(), 10, Duration::from_nanos(20));

    // All variants should pass through
    assert!(matches!(iter.next(), Some(Stream::Init)));
    assert!(matches!(iter.next(), Some(Stream::Ignore)));
    assert!(matches!(iter.next(), Some(Stream::Pending(_))));
    assert!(matches!(iter.next(), Some(Stream::Delayed(_))));
    assert!(matches!(iter.next(), Some(Stream::Next(42))));
}

#[test]
fn test_concurrent_queue_stream_iterator_concurrent_push() {
    use std::thread;

    let queue: Arc<ConcurrentQueue<Stream<u32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));
    let queue_clone = queue.clone();

    // Spawn a thread to push items after a delay
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        queue_clone.push(Stream::Next(100u32)).unwrap();
    });

    let mut iter =
        ConcurrentQueueStreamIterator::new(queue.clone(), 100, Duration::from_millis(10));

    // Should eventually receive the item pushed by the other thread
    let mut received = false;
    for _ in 0..20 {
        if let Some(Stream::Next(100)) = iter.next() {
            received = true;
            break;
        }
    }

    handle.join().unwrap();
    assert!(received, "Should receive item from concurrent thread");
}
