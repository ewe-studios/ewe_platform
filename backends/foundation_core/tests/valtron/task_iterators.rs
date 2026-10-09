// Integration tests for TaskIterator combinators
use foundation_core::valtron::{NoAction, Stream, TaskIteratorExt, TaskStatus};

// Simple test task iterator for unit tests
struct TestTask {
    items: Vec<TaskStatus<u32, String, NoAction>>,
}

impl TestTask {
    fn new(items: Vec<TaskStatus<u32, String, NoAction>>) -> Self {
        Self { items }
    }
}

impl Iterator for TestTask {
    type Item = TaskStatus<u32, String, NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.pop()
    }
}

#[test]
fn test_map_ready() {
    let items = vec![
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(10),
        TaskStatus::Ready(5),
    ];
    let task = TestTask::new(items);
    let mut mapped = task.map_ready(|x| x * 2);

    // pop() returns items in reverse order: 5, 10, Pending
    // After mapping: 10 (5*2), 20 (10*2), Pending
    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(10)));
    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(20)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::Pending("wait".to_string()))
    );
}

#[test]
fn test_map_pending() {
    let items = vec![
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(5),
    ];
    let task = TestTask::new(items);
    let mut mapped = task.map_pending(|s| s.len());

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(5)));
    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Pending(4)));
}

#[test]
fn test_filter_ready() {
    let items = vec![
        TaskStatus::Ready(3),
        TaskStatus::Ready(10),
        TaskStatus::Ready(5),
    ];
    let task = TestTask::new(items);
    let mut filtered = task.filter_ready(|x| *x > 5);

    // pop() returns: Ready(5), Ready(10), Ready(3)
    // filter: 5 > 5 = false → Ignore, 10 > 5 = true → Ready(10), 3 > 5 = false → Ignore
    assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ignore)); // 5 was filtered out
    assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ready(10)));
    assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ignore)); // 3 was filtered out
    assert_eq!(Iterator::next(&mut filtered), None);
}

#[test]
fn test_split_collector_map_sends_transformed_items() {
    let items = vec![
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(10),
        TaskStatus::Ready(3),
        TaskStatus::Ready(20),
    ];
    let task = TestTask::new(items);

    // Observer gets string representations of items > 5
    let (observer, mut continuation) =
        task.split_collector_map(|x| (*x > 5, Some(format!("val:{x}"))), 10);

    // Drive the continuation to completion
    while Iterator::next(&mut continuation).is_some() {}

    // Observer should have transformed items
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
    let items = vec![TaskStatus::Ready(10), TaskStatus::Ready(5)];
    let task = TestTask::new(items);

    // Matched but transform returns None for odd numbers → skipped
    let (observer, mut continuation) = task.split_collector_map(
        |x| {
            (
                true,
                if x % 2 == 0 {
                    Some(u64::from(*x))
                } else {
                    None
                },
            )
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

/// `split_collect_one_map` uses a depth-1 (queue_size = 1) observer queue. Since
/// F45 Part C1 replaced `force_push` (drop-oldest) with a vacancy-park, a full
/// queue backpressures the source instead of dropping — so the continuation and
/// observer must be driven in lockstep, and every match is delivered (zero loss).
#[test]
fn test_split_collect_one_map() {
    // TestTask pops in reverse, so this yields 20, 10, 3.
    let items = vec![
        TaskStatus::Ready(3),
        TaskStatus::Ready(10),
        TaskStatus::Ready(20),
    ];
    let task = TestTask::new(items);

    let (mut observer, mut continuation) =
        task.split_collect_one_map(|x| (*x > 5, Some(x.to_string())));

    // Lockstep drive: drain the depth-1 queue between continuation steps so a
    // parked source can advance.
    let mut collected: Vec<String> = Vec::new();
    loop {
        while let Some(stream) = Iterator::next(&mut observer) {
            match stream {
                Stream::Next(v) => collected.push(v),
                _ => break,
            }
        }
        match Iterator::next(&mut continuation) {
            Some(_) => {}
            None => break,
        }
    }
    for stream in &mut observer {
        if let Stream::Next(v) = stream {
            collected.push(v);
        }
    }

    // Both matches (20, 10) are delivered in order — zero loss.
    assert_eq!(collected, vec!["20".to_string(), "10".to_string()]);
}

#[test]
fn test_stream_collect() {
    let items = vec![
        TaskStatus::Ready(2),
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(1),
    ];
    let task = TestTask::new(items);
    let mut collected = task.stream_collect();

    // First Ready is collected, returns Ignore
    assert_eq!(Iterator::next(&mut collected), Some(TaskStatus::Ignore));

    // Should pass through Pending
    assert_eq!(
        Iterator::next(&mut collected),
        Some(TaskStatus::Pending("wait".to_string()))
    );

    // Second Ready is collected, returns Ignore
    assert_eq!(Iterator::next(&mut collected), Some(TaskStatus::Ignore));

    // Should yield collected Vec at the end
    match Iterator::next(&mut collected) {
        Some(TaskStatus::Ready(vec)) => {
            assert_eq!(vec.len(), 2);
            assert!(vec.contains(&1));
            assert!(vec.contains(&2));
        }
        other => panic!("Expected Ready(Vec), got {other:?}"),
    }

    // Should be done after yielding the collected result
    assert_eq!(Iterator::next(&mut collected), None);
}

#[test]
fn test_map_iter_flattens_nested_iterators() {
    // Test using the extension method with a simple test task
    struct VecTask {
        items: std::vec::IntoIter<TaskStatus<Vec<u32>, String, NoAction>>,
    }

    impl Iterator for VecTask {
        type Item = TaskStatus<Vec<u32>, String, NoAction>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        TaskStatus::Ready(vec![1u32, 2u32]),
        TaskStatus::Ready(vec![3u32, 4u32, 5u32]),
    ];
    let task = VecTask {
        items: items.into_iter(),
    };

    let mut flattened: foundation_core::valtron::TMapIter<
        _,
        _,
        _,
        u32,
        String,
        NoAction,
        Vec<u32>,
        String,
        NoAction,
    > = task.map_iter(|vec| {
        vec.into_iter()
            .map(TaskStatus::Ready)
            .collect::<Vec<_>>()
            .into_iter()
    });

    // Should flatten: 1, 2, 3, 4, 5
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(2))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(3))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(4))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(5))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}

#[test]
fn test_map_iter_passes_through_pending() {
    // Test using the extension method with a simple test task
    struct VecTask {
        items: std::vec::IntoIter<TaskStatus<Vec<u32>, String, NoAction>>,
    }

    impl Iterator for VecTask {
        type Item = TaskStatus<Vec<u32>, String, NoAction>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    let items = vec![
        TaskStatus::Ready(vec![1u32]),
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(vec![2u32, 3u32]),
    ];
    let task = VecTask {
        items: items.into_iter(),
    };

    let mut flattened: foundation_core::valtron::TMapIter<
        _,
        _,
        _,
        u32,
        String,
        NoAction,
        Vec<u32>,
        String,
        NoAction,
    > = task.map_iter(|vec| {
        vec.into_iter()
            .map(TaskStatus::Ready)
            .collect::<Vec<_>>()
            .into_iter()
    });

    // Should flatten with pending passed through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(ref s)) if s == "wait"
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(2))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(3))
    ));
    assert_eq!(Iterator::next(&mut flattened), None);
}
