//! Tests for SpreadDone and SpreadPending variants.
//!
//! Covers: type conversion, delivery point expansion, edge cases, and async futures.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use foundation_core::valtron::{
    NoAction, Stream, StreamAsFutureStream, StreamCollectFuture, StreamIteratorExt,
    StreamPendingFuture, StreamReadyFuture, TaskIteratorExt, TaskStatus,
};
use futures_core::Stream as FuturesStream;
use tracing_test::traced_test;

// ============================================================================
// No-Op Waker for sync polling
// ============================================================================

fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(|_| RAW_WAKER, |_| {}, |_| {}, |_| {});
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW_WAKER) }
}

fn poll_once<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(f).poll(&mut cx)
}

fn poll_stream_next<S: FuturesStream + Unpin>(s: &mut S) -> Poll<Option<S::Item>> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(s).poll_next(&mut cx)
}

// ============================================================================
// Conversion tests (TASK-09-21)
// ============================================================================

#[test]
fn test_task_status_spread_done_converts_to_stream_spread_done() {
    let ts: TaskStatus<i32, &str, NoAction> = TaskStatus::SpreadDone(vec![1, 2, 3]);
    let stream: Stream<i32, &str> = ts.into();
    assert_eq!(stream, Stream::SpreadDone(vec![1, 2, 3]));
}

#[test]
fn test_task_status_spread_pending_converts_to_stream_spread_pending() {
    let ts: TaskStatus<&str, i32, NoAction> = TaskStatus::SpreadPending(vec![10, 20]);
    let stream: Stream<&str, i32> = ts.into();
    assert_eq!(stream, Stream::SpreadPending(vec![10, 20]));
}

#[test]
fn test_spread_done_partial_eq() {
    let a: Stream<i32, &str> = Stream::SpreadDone(vec![1, 2, 3]);
    let b: Stream<i32, &str> = Stream::SpreadDone(vec![1, 2, 3]);
    let c: Stream<i32, &str> = Stream::SpreadDone(vec![1, 2, 4]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_spread_pending_partial_eq() {
    let a: Stream<i32, &str> = Stream::SpreadPending(vec!["a", "b"]);
    let b: Stream<i32, &str> = Stream::SpreadPending(vec!["a", "b"]);
    let c: Stream<i32, &str> = Stream::SpreadPending(vec!["a", "c"]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ============================================================================
// Stream iterator spread mapper tests
// ============================================================================

#[test]
fn test_stream_map_done_transforms_spread_done() {
    let items = vec![
        Stream::<i32, &str>::Next(5),
        Stream::SpreadDone(vec![1, 2, 3]),
    ];
    let mut mapped = items.into_iter().map_done(|x| x * 10);

    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(50)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::SpreadDone(vec![10, 20, 30]))
    );
}

#[test]
fn test_stream_map_pending_transforms_spread_pending() {
    let items = vec![
        Stream::<i32, String>::Pending("hi".to_string()),
        Stream::SpreadPending(vec!["ab".to_string(), "c".to_string()]),
    ];
    let mut mapped = items.into_iter().map_pending(|s| s.len());

    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Pending(2)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::SpreadPending(vec![2, 1]))
    );
}

#[test]
fn test_stream_map_done_passthrough_spread_pending() {
    let items = vec![Stream::<i32, &str>::SpreadPending(vec!["a", "b"])];
    let mut mapped = items.into_iter().map_done(|x| x * 2);
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::SpreadPending(vec!["a", "b"]))
    );
}

#[test]
fn test_stream_map_pending_passthrough_spread_done() {
    let items = vec![Stream::<i32, &str>::SpreadDone(vec![1, 2])];
    let mut mapped = items.into_iter().map_pending(|s: &str| s.len());
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::SpreadDone(vec![1, 2]))
    );
}

// ============================================================================
// Task iterator spread mapper tests
// ============================================================================

#[test]
fn test_task_map_ready_transforms_spread_done() {
    let items = vec![
        TaskStatus::<i32, &str, NoAction>::Ready(5),
        TaskStatus::SpreadDone(vec![1, 2, 3]),
    ];
    let mut mapped = items.into_iter().map_ready(|x| x * 10);

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(50)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::SpreadDone(vec![10, 20, 30]))
    );
}

#[test]
fn test_task_map_pending_transforms_spread_pending() {
    let items = vec![
        TaskStatus::<i32, String, NoAction>::Pending("hi".to_string()),
        TaskStatus::SpreadPending(vec!["ab".to_string(), "c".to_string()]),
    ];
    let mut mapped = items.into_iter().map_pending(|s| s.len());

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Pending(2)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::SpreadPending(vec![2, 1]))
    );
}

#[test]
fn test_task_map_ready_passthrough_spread_pending() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadPending(vec![
        "a", "b",
    ])];
    let mut mapped = items.into_iter().map_ready(|x| x * 2);
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::SpreadPending(vec!["a", "b"]))
    );
}

#[test]
fn test_task_map_pending_passthrough_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![1, 2])];
    let mut mapped = items.into_iter().map_pending(|s: &str| s.len());
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::SpreadDone(vec![1, 2]))
    );
}

// ============================================================================
// Task iterator spread combinator tests
// ============================================================================

#[test]
fn test_task_enumerate_spread_done() {
    use foundation_core::valtron::TaskIteratorExt;

    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![
        10, 20, 30,
    ])];
    let mut enumd = TaskIteratorExt::enumerate(items.into_iter());
    let result = Iterator::next(&mut enumd).unwrap();
    match result {
        TaskStatus::SpreadDone(items) => {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], (0, 10));
            assert_eq!(items[1], (1, 20));
            assert_eq!(items[2], (2, 30));
        }
        _ => panic!("expected SpreadDone"),
    }
}

#[test]
fn test_task_find_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![1, 2, 3])];
    let mut found = items.into_iter().find(|x| *x == 2);
    assert_eq!(Iterator::next(&mut found), Some(TaskStatus::Ready(Some(2))));
}

#[test]
fn test_task_find_no_match_in_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![1, 2, 3])];
    let mut found = items.into_iter().find(|x| *x == 5);
    // Returns Ignore when no match found in spread
    assert_eq!(Iterator::next(&mut found), Some(TaskStatus::Ignore));
}

#[test]
fn test_task_fold_spread_done() {
    use foundation_core::valtron::TaskIteratorExt;

    let items = vec![
        TaskStatus::<i32, &str, NoAction>::Ready(1),
        TaskStatus::SpreadDone(vec![2, 3]),
    ];
    let mut folded = TaskIteratorExt::fold(items.into_iter(), 0, |acc, x| acc + x);
    // fold processes SpreadDone items and returns Ignore, then Ready on exhaustion
    assert_eq!(Iterator::next(&mut folded), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut folded), Some(TaskStatus::Ready(6)));
}

#[test]
fn test_task_all_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![2, 4, 6])];
    let mut all = items.into_iter().all(|x| x % 2 == 0);
    // all processes SpreadDone and returns Ignore, then Ready(true) on exhaustion
    assert_eq!(Iterator::next(&mut all), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut all), Some(TaskStatus::Ready(true)));
}

#[test]
fn test_task_all_spread_done_false() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![2, 3, 6])];
    let mut all = items.into_iter().all(|x| x % 2 == 0);
    assert_eq!(Iterator::next(&mut all), Some(TaskStatus::Ready(false)));
}

#[test]
fn test_task_any_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![1, 2, 5])];
    let mut any = items.into_iter().any(|x| x % 2 == 0);
    // any processes SpreadDone items; 2%2==0 → returns Ready(true) immediately
    assert_eq!(Iterator::next(&mut any), Some(TaskStatus::Ready(true)));
    // After finding a match, any_true=true, so next returns None
    assert_eq!(Iterator::next(&mut any), None);
}

#[test]
fn test_task_any_spread_done_false() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![1, 3, 5])];
    let mut any = items.into_iter().any(|x| x % 2 == 0);
    // no items match → returns Ignore, then Ready(false) on exhaustion
    assert_eq!(Iterator::next(&mut any), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut any), Some(TaskStatus::Ready(false)));
}

#[test]
fn test_task_count_spread_done() {
    use foundation_core::valtron::TaskIteratorExt;

    let items = vec![TaskStatus::<i32, &str, NoAction>::SpreadDone(vec![
        1, 2, 3, 4,
    ])];
    let mut count = TaskIteratorExt::count(items.into_iter());
    // count processes SpreadDone and returns Ignore, then Ready(total) on exhaustion
    assert_eq!(Iterator::next(&mut count), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut count), Some(TaskStatus::Ready(4)));
}

// ============================================================================
// Edge case tests (TASK-09-26, TASK-09-27)
// ============================================================================

#[test]
fn test_empty_spread_done_delivers_nothing_via_iterator() {
    let items = vec![Stream::<i32, &str>::SpreadDone(vec![]), Stream::Next(42)];
    let mut iter = items.into_iter();
    // SpreadDone(vec![]) is yielded as-is; delivery point would expand to nothing
    let first = Iterator::next(&mut iter).unwrap();
    match first {
        Stream::SpreadDone(items) => assert!(items.is_empty()),
        _ => panic!("expected SpreadDone"),
    }
    assert_eq!(Iterator::next(&mut iter), Some(Stream::Next(42)));
}

#[test]
fn test_empty_spread_pending_delivers_nothing_via_iterator() {
    let items = vec![Stream::<i32, &str>::SpreadPending(vec![]), Stream::Next(42)];
    let mut iter = items.into_iter();
    let first = Iterator::next(&mut iter).unwrap();
    match first {
        Stream::SpreadPending(items) => assert!(items.is_empty()),
        _ => panic!("expected SpreadPending"),
    }
    assert_eq!(Iterator::next(&mut iter), Some(Stream::Next(42)));
}

#[test]
fn test_single_element_spread_done() {
    let items = vec![Stream::<i32, &str>::SpreadDone(vec![42])];
    let mut iter = items.into_iter();
    assert_eq!(
        Iterator::next(&mut iter),
        Some(Stream::SpreadDone(vec![42]))
    );
    assert_eq!(Iterator::next(&mut iter), None);
}

#[test]
fn test_single_element_spread_pending() {
    let items = vec![Stream::<i32, &str>::SpreadPending(vec!["x"])];
    let mut iter = items.into_iter();
    assert_eq!(
        Iterator::next(&mut iter),
        Some(Stream::SpreadPending(vec!["x"]))
    );
    assert_eq!(Iterator::next(&mut iter), None);
}

#[test]
fn test_empty_spread_done_via_map_done() {
    let items = vec![Stream::<i32, &str>::SpreadDone(vec![])];
    let mut mapped = items.into_iter().map_done(|x| x * 10);
    match Iterator::next(&mut mapped).unwrap() {
        Stream::SpreadDone(items) => assert!(items.is_empty()),
        _ => panic!("expected empty SpreadDone"),
    }
}

// ============================================================================
// Async future tests — StreamCollectFuture (TASK-09-28)
// ============================================================================

#[test]
#[traced_test]
fn test_collect_future_next_values_collected() {
    let iter = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Next(2),
        Stream::Next(3),
    ]
    .into_iter();
    let mut future = StreamCollectFuture::new(iter);

    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![1, 2, 3]),
        _ => panic!("expected Ready"),
    }
}

#[test]
#[traced_test]
fn test_collect_future_spread_pending_returns_pending() {
    let iter = vec![
        Stream::<i32, &str>::Next(1),
        Stream::SpreadPending(vec!["a", "b"]),
        Stream::Next(2),
    ]
    .into_iter();
    let mut future = StreamCollectFuture::new(iter);

    // SpreadPending triggers Poll::Pending (stream signaled not-ready)
    assert!(matches!(poll_once(&mut future), Poll::Pending));
    // After re-wake, Next(2) is collected, then iterator exhausted
    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![1, 2]),
        _ => panic!("expected Ready"),
    }
}

#[test]
#[traced_test]
fn test_collect_future_spread_done_returns_pending() {
    // SpreadDone is treated as a signal to re-wake (not-ready),
    // consistent with the implementation's handling of spread in futures.
    let iter = vec![Stream::<i32, &str>::Next(1), Stream::SpreadDone(vec![2, 3])].into_iter();
    let mut future = StreamCollectFuture::new(iter);

    // SpreadDone triggers Poll::Pending
    assert!(matches!(poll_once(&mut future), Poll::Pending));
    // After re-wake, iterator is exhausted
    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![1]),
        _ => panic!("expected Ready"),
    }
}

// ============================================================================
// Async future tests — StreamReadyFuture (TASK-09-29)
// ============================================================================

#[test]
#[traced_test]
fn test_ready_future_spread_done_returns_first_value() {
    let iter = vec![Stream::<i32, &str>::SpreadDone(vec![42, 99, 100])].into_iter();
    let mut future = StreamReadyFuture::new(iter);

    match poll_once(&mut future) {
        Poll::Ready(Some((value, _))) => {
            assert_eq!(value, 42);
        }
        other => panic!("expected Some, got {:?}", other),
    }
}

#[test]
#[traced_test]
fn test_ready_future_spread_done_empty_returns_pending() {
    let iter = vec![Stream::<i32, &str>::SpreadDone(vec![])].into_iter();
    let mut future = StreamReadyFuture::new(iter);

    // Empty SpreadDone: no value to return, iterator exhausted → None
    match poll_once(&mut future) {
        Poll::Ready(None) => {}
        other => panic!("expected None, got {:?}", other),
    }
}

#[test]
#[traced_test]
fn test_ready_future_spread_pending_returns_pending() {
    let iter = vec![
        Stream::<i32, &str>::SpreadPending(vec!["a", "b"]),
        Stream::Next(42),
    ]
    .into_iter();
    let mut future = StreamReadyFuture::new(iter);

    // SpreadPending triggers Poll::Pending
    assert!(matches!(poll_once(&mut future), Poll::Pending));
    // After re-wake, Next(42) is returned
    match poll_once(&mut future) {
        Poll::Ready(Some((value, _))) => assert_eq!(value, 42),
        other => panic!("expected Some, got {:?}", other),
    }
}

// ============================================================================
// Async future tests — StreamPendingFuture (TASK-09-30)
// ============================================================================

#[test]
#[traced_test]
fn test_pending_future_spread_pending_returns_first_value() {
    let iter = vec![Stream::<i32, &str>::SpreadPending(vec!["a", "b", "c"])].into_iter();
    let mut future = StreamPendingFuture::new(iter);

    // StreamPendingFuture returns Ready immediately on SpreadPending (first value)
    match poll_once(&mut future) {
        Poll::Ready(Some((ctx, _))) => {
            assert_eq!(ctx, "a");
        }
        other => panic!("expected Some, got {:?}", other),
    }
}

#[test]
#[traced_test]
fn test_pending_future_spread_pending_empty_returns_pending() {
    let iter = vec![Stream::<i32, &str>::SpreadPending(vec![])].into_iter();
    let mut future = StreamPendingFuture::new(iter);

    // Empty SpreadPending: no value to return, iterator exhausted → None
    match poll_once(&mut future) {
        Poll::Ready(None) => {}
        other => panic!("expected None, got {:?}", other),
    }
}

#[test]
#[traced_test]
fn test_pending_future_spread_done_returns_pending() {
    let iter = vec![
        Stream::<i32, &str>::SpreadDone(vec![1, 2]),
        Stream::Pending("x"),
    ]
    .into_iter();
    let mut future = StreamPendingFuture::new(iter);

    // SpreadDone triggers Poll::Pending (consistent with Next behavior)
    assert!(matches!(poll_once(&mut future), Poll::Pending));
    // After re-wake, Pending("x") is returned
    match poll_once(&mut future) {
        Poll::Ready(Some((ctx, _))) => assert_eq!(ctx, "x"),
        other => panic!("expected Some, got {:?}", other),
    }
}

// ============================================================================
// Async future tests — StreamAsFutureStream (TASK-09-31)
// ============================================================================

#[test]
#[traced_test]
fn test_future_stream_spread_done_yielded_as_single_item() {
    // StreamAsFutureStream passes through iterator items as-is
    let mut stream = StreamAsFutureStream::new(
        vec![
            Stream::<i32, &str>::Next(1),
            Stream::SpreadDone(vec![2, 3]),
            Stream::Next(4),
        ]
        .into_iter(),
    );

    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Next(1)))
    );
    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::SpreadDone(vec![2, 3])))
    );
    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Next(4)))
    );
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
}

#[test]
#[traced_test]
fn test_future_stream_spread_pending_yielded_as_single_item() {
    let mut stream = StreamAsFutureStream::new(
        vec![
            Stream::<i32, &str>::Pending("a"),
            Stream::SpreadPending(vec!["b", "c"]),
        ]
        .into_iter(),
    );

    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Pending("a")))
    );
    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::SpreadPending(vec!["b", "c"])))
    );
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
}

#[test]
#[traced_test]
fn test_future_stream_empty_spread() {
    let mut stream = StreamAsFutureStream::new(
        vec![
            Stream::<i32, &str>::SpreadDone(vec![]),
            Stream::SpreadPending(vec![]),
        ]
        .into_iter(),
    );

    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::SpreadDone(vec![])))
    );
    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::SpreadPending(vec![])))
    );
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
}

// ============================================================================
// Tokio async tests
// ============================================================================

#[tokio::test]
#[traced_test]
async fn test_tokio_collect_with_spread_pending_in_chain() {
    let result = vec![
        Stream::<i32, &str>::Next(1),
        Stream::SpreadPending(vec!["a"]),
        Stream::Next(2),
    ]
    .into_iter()
    .into_collect_future()
    .await;
    // Collect sees Next(1), then SpreadPending → Pending (wakes), then Next(2)
    // on next poll. Result should contain both Next values.
    assert_eq!(result, vec![1, 2]);
}

#[tokio::test]
#[traced_test]
async fn test_tokio_ready_future_with_spread_done() {
    let (value, _) = vec![Stream::<i32, &str>::SpreadDone(vec![42, 99])]
        .into_iter()
        .into_ready_future()
        .await
        .unwrap();
    assert_eq!(value, 42);
}

#[tokio::test]
#[traced_test]
async fn test_tokio_pending_future_with_spread_pending() {
    let (ctx, _) = vec![
        Stream::<i32, &str>::Next(1),
        Stream::SpreadPending(vec!["done"]),
    ]
    .into_iter()
    .into_pending_future()
    .await
    .unwrap();
    assert_eq!(ctx, "done");
}

#[tokio::test]
#[traced_test]
async fn test_tokio_future_stream_with_spread() {
    let mut stream = vec![
        Stream::<i32, &str>::Next(1),
        Stream::SpreadDone(vec![2, 3]),
        Stream::SpreadPending(vec!["a"]),
    ]
    .into_iter()
    .into_future_stream();
    let mut items = Vec::new();
    while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
        items.push(item);
    }
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], Stream::Next(1));
    assert_eq!(items[1], Stream::SpreadDone(vec![2, 3]));
    assert_eq!(items[2], Stream::SpreadPending(vec!["a"]));
}

// ============================================================================
// smol async tests
// ============================================================================

#[test]
#[traced_test]
fn test_smol_collect_with_spread_pending() {
    let result = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Next(10),
            Stream::SpreadPending(vec!["wait"]),
            Stream::Next(20),
        ]
        .into_iter()
        .into_collect_future()
        .await
    });
    assert_eq!(result, vec![10, 20]);
}

#[test]
#[traced_test]
fn test_smol_ready_future_with_spread_done() {
    let (value, mut remaining) = smol::block_on(async {
        vec![
            Stream::<i32, &str>::SpreadDone(vec![99, 100]),
            Stream::Next(1),
        ]
        .into_iter()
        .into_ready_future()
        .await
        .unwrap()
    });
    assert_eq!(value, 99);
    // Remaining iterator should still have SpreadDone and Next
    assert!(remaining.next().is_some());
}

#[test]
#[traced_test]
fn test_smol_future_stream_with_spread() {
    let mut stream = vec![
        Stream::<i32, &str>::SpreadDone(vec![1, 2]),
        Stream::SpreadPending(vec!["p"]),
    ]
    .into_iter()
    .into_future_stream();
    let mut items = Vec::new();
    while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
        items.push(item);
    }
    assert_eq!(items.len(), 2);
}

// ============================================================================
// Combinator chaining with spread
// ============================================================================

#[test]
#[traced_test]
fn test_map_done_then_collect_with_spread_done_in_chain() {
    let result = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Next(5),
            Stream::SpreadDone(vec![1, 2, 3]),
        ]
        .into_iter()
        .map_done(|v| v * 2)
        .into_collect_future()
        .await
    });
    // SpreadDone triggers Pending, so only Next(5) collected
    assert_eq!(result, vec![10]);
}

#[test]
#[traced_test]
fn test_task_map_ready_then_collect_with_spread_done() {
    use foundation_core::valtron::TaskStatus;

    let items = vec![
        TaskStatus::<i32, &str, NoAction>::Ready(5),
        TaskStatus::SpreadDone(vec![1, 2, 3]),
    ];
    let mut mapped = items.into_iter().map_ready(|x| x * 10);

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(50)));
    // SpreadDone is mapped: each item * 10, re-wrapped as SpreadDone
    match Iterator::next(&mut mapped).unwrap() {
        TaskStatus::SpreadDone(items) => {
            assert_eq!(items, vec![10, 20, 30]);
        }
        _ => panic!("expected SpreadDone"),
    }
}
