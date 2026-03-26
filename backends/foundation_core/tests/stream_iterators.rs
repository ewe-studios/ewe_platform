// Integration tests for StreamIterator combinators
use foundation_core::valtron::{
    CollectAll, MapAllDone, MapAllPendingAndDone, Stream, StreamIterator, StreamIteratorExt,
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

impl StreamIterator for TestStream {
    type D = u32;
    type P = String;
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

impl<D, P> StreamIterator for SimpleStream<D, P> {
    type D = D;
    type P = P;
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
        other => panic!("Expected Next(Vec), got {:?}", other),
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
            Stream::Next(v) if *v > 5 => (true, Some(Stream::Next(format!("val:{}", v)))),
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
            Stream::Next(v) if v % 2 == 0 => (true, Some(Stream::Next(*v as u64))),
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
        match item {
            Stream::Next(values) => collected_values.extend(values),
            Stream::Pending(_) | Stream::Ignore => {}
            _ => {}
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
        match item {
            Stream::Next(sum) => {
                assert_eq!(sum, 11); // 1 + 10
                got_result = true;
            }
            Stream::Pending(_) | Stream::Init => {}
            _ => {}
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
        match item {
            Stream::Next(count) => {
                assert_eq!(count, 2); // Two states
                got_result = true;
            }
            _ => {}
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

    impl StreamIterator for VecStream {
        type D = Vec<u32>;
        type P = String;
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

    impl StreamIterator for VecStream {
        type D = Vec<u32>;
        type P = String;
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

    impl StreamIterator for VecStream {
        type D = Vec<u32>;
        type P = String;
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

    impl StreamIterator for VecStream {
        type D = Vec<u32>;
        type P = String;
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

    impl StreamIterator for NumStream {
        type D = u32;
        type P = String;
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

    impl StreamIterator for VecStream {
        type D = u32;
        type P = Vec<String>;
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

    impl StreamIterator for NumStream {
        type D = u32;
        type P = u32;
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
