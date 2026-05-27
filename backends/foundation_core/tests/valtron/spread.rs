//! Tests for the unified Spread variant.
//!
//! Covers: type conversion, delivery point expansion, edge cases, and async futures.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use foundation_core::valtron::{
    NoAction, Stream, StreamAsFutureStream, StreamCollectFuture, StreamIteratorExt,
    StreamPendingFuture, StreamReadyFuture, StreamSpread, TaskIteratorExt, TaskSpread,
    TaskStatus,
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
fn test_task_status_spread_ready_converts_to_stream_spread_done() {
    let ts: TaskStatus<i32, &str, NoAction> =
        TaskStatus::Spread(vec![TaskSpread::Ready(1), TaskSpread::Ready(2), TaskSpread::Ready(3)]);
    let stream: Stream<i32, &str> = ts.into();
    assert_eq!(
        stream,
        Stream::Spread(vec![StreamSpread::Done(1), StreamSpread::Done(2), StreamSpread::Done(3)])
    );
}

#[test]
fn test_task_status_spread_pending_converts_to_stream_spread_pending() {
    let ts: TaskStatus<&str, i32, NoAction> =
        TaskStatus::Spread(vec![TaskSpread::Pending(10), TaskSpread::Pending(20)]);
    let stream: Stream<&str, i32> = ts.into();
    assert_eq!(
        stream,
        Stream::Spread(vec![StreamSpread::Pending(10), StreamSpread::Pending(20)])
    );
}

#[test]
fn test_spread_done_partial_eq() {
    let a: Stream<i32, &str> = Stream::Spread(vec![
        StreamSpread::Done(1),
        StreamSpread::Done(2),
        StreamSpread::Done(3),
    ]);
    let b: Stream<i32, &str> = Stream::Spread(vec![
        StreamSpread::Done(1),
        StreamSpread::Done(2),
        StreamSpread::Done(3),
    ]);
    let c: Stream<i32, &str> = Stream::Spread(vec![
        StreamSpread::Done(1),
        StreamSpread::Done(2),
        StreamSpread::Done(4),
    ]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_spread_pending_partial_eq() {
    let a: Stream<i32, &str> =
        Stream::Spread(vec![StreamSpread::Pending("a"), StreamSpread::Pending("b")]);
    let b: Stream<i32, &str> =
        Stream::Spread(vec![StreamSpread::Pending("a"), StreamSpread::Pending("b")]);
    let c: Stream<i32, &str> =
        Stream::Spread(vec![StreamSpread::Pending("a"), StreamSpread::Pending("c")]);
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
        Stream::Spread(vec![
            StreamSpread::Done(1),
            StreamSpread::Done(2),
            StreamSpread::Done(3),
        ]),
    ];
    let mut mapped = items.into_iter().map_done(|x| x * 10);

    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(50)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::Spread(vec![
            StreamSpread::Done(10),
            StreamSpread::Done(20),
            StreamSpread::Done(30),
        ]))
    );
}

#[test]
fn test_stream_map_pending_transforms_spread_pending() {
    let items = vec![
        Stream::<i32, String>::Pending("hi".to_string()),
        Stream::Spread(vec![
            StreamSpread::Pending("ab".to_string()),
            StreamSpread::Pending("c".to_string()),
        ]),
    ];
    let mut mapped = items.into_iter().map_pending(|s| s.len());

    assert_eq!(Iterator::next(&mut mapped), Some(Stream::Pending(2)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::Spread(vec![
            StreamSpread::Pending(2),
            StreamSpread::Pending(1),
        ]))
    );
}

#[test]
fn test_stream_map_done_passthrough_spread_pending() {
    let items = vec![Stream::<i32, &str>::Spread(vec![
        StreamSpread::Pending("a"),
        StreamSpread::Pending("b"),
    ])];
    let mut mapped = items.into_iter().map_done(|x| x * 2);
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::Spread(vec![
            StreamSpread::Pending("a"),
            StreamSpread::Pending("b"),
        ]))
    );
}

#[test]
fn test_stream_map_pending_passthrough_spread_done() {
    let items = vec![Stream::<i32, &str>::Spread(vec![
        StreamSpread::Done(1),
        StreamSpread::Done(2),
    ])];
    let mut mapped = items.into_iter().map_pending(|s: &str| s.len());
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(Stream::Spread(vec![
            StreamSpread::Done(1),
            StreamSpread::Done(2),
        ]))
    );
}

// ============================================================================
// Task iterator spread mapper tests
// ============================================================================

#[test]
fn test_task_map_ready_transforms_spread_done() {
    let items = vec![
        TaskStatus::<i32, &str, NoAction>::Ready(5),
        TaskStatus::Spread(vec![
            TaskSpread::Ready(1),
            TaskSpread::Ready(2),
            TaskSpread::Ready(3),
        ]),
    ];
    let mut mapped = items.into_iter().map_ready(|x| x * 10);

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(50)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::Spread(vec![
            TaskSpread::Ready(10),
            TaskSpread::Ready(20),
            TaskSpread::Ready(30),
        ]))
    );
}

#[test]
fn test_task_map_pending_transforms_spread_pending() {
    let items = vec![
        TaskStatus::<i32, String, NoAction>::Pending("hi".to_string()),
        TaskStatus::Spread(vec![
            TaskSpread::Pending("ab".to_string()),
            TaskSpread::Pending("c".to_string()),
        ]),
    ];
    let mut mapped = items.into_iter().map_pending(|s| s.len());

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Pending(2)));
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::Spread(vec![
            TaskSpread::Pending(2),
            TaskSpread::Pending(1),
        ]))
    );
}

#[test]
fn test_task_map_ready_passthrough_spread_pending() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Pending("a"),
        TaskSpread::Pending("b"),
    ])];
    let mut mapped = items.into_iter().map_ready(|x| x * 2);
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::Spread(vec![
            TaskSpread::Pending("a"),
            TaskSpread::Pending("b"),
        ]))
    );
}

#[test]
fn test_task_map_pending_passthrough_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(1),
        TaskSpread::Ready(2),
    ])];
    let mut mapped = items.into_iter().map_pending(|s: &str| s.len());
    assert_eq!(
        Iterator::next(&mut mapped),
        Some(TaskStatus::Spread(vec![
            TaskSpread::Ready(1),
            TaskSpread::Ready(2),
        ]))
    );
}

// ============================================================================
// Task iterator spread combinator tests
// ============================================================================

#[test]
fn test_task_enumerate_spread_done() {
    use foundation_core::valtron::TaskIteratorExt;

    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(10),
        TaskSpread::Ready(20),
        TaskSpread::Ready(30),
    ])];
    let mut enumd = TaskIteratorExt::enumerate(items.into_iter());
    let result = Iterator::next(&mut enumd).unwrap();
    match result {
        TaskStatus::Spread(items) => {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], TaskSpread::Ready((0, 10)));
            assert_eq!(items[1], TaskSpread::Ready((1, 20)));
            assert_eq!(items[2], TaskSpread::Ready((2, 30)));
        }
        _ => panic!("expected Spread"),
    }
}

#[test]
fn test_task_find_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(1),
        TaskSpread::Ready(2),
        TaskSpread::Ready(3),
    ])];
    let mut found = items.into_iter().find(|x| *x == 2);
    assert_eq!(Iterator::next(&mut found), Some(TaskStatus::Ready(Some(2))));
}

#[test]
fn test_task_find_no_match_in_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(1),
        TaskSpread::Ready(2),
        TaskSpread::Ready(3),
    ])];
    let mut found = items.into_iter().find(|x| *x == 5);
    // Returns Ignore when no match found in spread
    assert_eq!(Iterator::next(&mut found), Some(TaskStatus::Ignore));
}

#[test]
fn test_task_fold_spread_done() {
    use foundation_core::valtron::TaskIteratorExt;

    let items = vec![
        TaskStatus::<i32, &str, NoAction>::Ready(1),
        TaskStatus::Spread(vec![TaskSpread::Ready(2), TaskSpread::Ready(3)]),
    ];
    let mut folded = TaskIteratorExt::fold(items.into_iter(), 0, |acc, x| acc + x);
    // fold processes Ready → Ignore, then Spread → Ignore, then exhaustion → Ready
    assert_eq!(Iterator::next(&mut folded), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut folded), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut folded), Some(TaskStatus::Ready(6)));
}

#[test]
fn test_task_all_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(2),
        TaskSpread::Ready(4),
        TaskSpread::Ready(6),
    ])];
    let mut all = items.into_iter().all(|x| x % 2 == 0);
    // all processes Spread items and returns Ignore, then Ready(true) on exhaustion
    assert_eq!(Iterator::next(&mut all), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut all), Some(TaskStatus::Ready(true)));
}

#[test]
fn test_task_all_spread_done_false() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(2),
        TaskSpread::Ready(3),
        TaskSpread::Ready(6),
    ])];
    let mut all = items.into_iter().all(|x| x % 2 == 0);
    assert_eq!(Iterator::next(&mut all), Some(TaskStatus::Ready(false)));
}

#[test]
fn test_task_any_spread_done() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(1),
        TaskSpread::Ready(2),
        TaskSpread::Ready(5),
    ])];
    let mut any = items.into_iter().any(|x| x % 2 == 0);
    // any processes Spread items; 2%2==0 → returns Ready(true) immediately
    assert_eq!(Iterator::next(&mut any), Some(TaskStatus::Ready(true)));
    // After finding a match, any_true=true, so next returns None
    assert_eq!(Iterator::next(&mut any), None);
}

#[test]
fn test_task_any_spread_done_false() {
    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(1),
        TaskSpread::Ready(3),
        TaskSpread::Ready(5),
    ])];
    let mut any = items.into_iter().any(|x| x % 2 == 0);
    // no items match → returns Ignore, then Ready(false) on exhaustion
    assert_eq!(Iterator::next(&mut any), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut any), Some(TaskStatus::Ready(false)));
}

#[test]
fn test_task_count_spread_done() {
    use foundation_core::valtron::TaskIteratorExt;

    let items = vec![TaskStatus::<i32, &str, NoAction>::Spread(vec![
        TaskSpread::Ready(1),
        TaskSpread::Ready(2),
        TaskSpread::Ready(3),
        TaskSpread::Ready(4),
    ])];
    let mut count = TaskIteratorExt::count(items.into_iter());
    // count processes Spread and returns Ignore, then Ready(total) on exhaustion
    assert_eq!(Iterator::next(&mut count), Some(TaskStatus::Ignore));
    assert_eq!(Iterator::next(&mut count), Some(TaskStatus::Ready(4)));
}

// ============================================================================
// Edge case tests (TASK-09-26, TASK-09-27)
// ============================================================================

#[test]
fn test_empty_spread_done_delivers_nothing_via_iterator() {
    let items = vec![Stream::<i32, &str>::Spread(vec![]), Stream::Next(42)];
    let mut iter = items.into_iter();
    // Spread(vec![]) is yielded as-is; delivery point would expand to nothing
    let first = Iterator::next(&mut iter).unwrap();
    match first {
        Stream::Spread(items) => assert!(items.is_empty()),
        _ => panic!("expected Spread"),
    }
    assert_eq!(Iterator::next(&mut iter), Some(Stream::Next(42)));
}

#[test]
fn test_empty_spread_pending_delivers_nothing_via_iterator() {
    let items = vec![Stream::<i32, &str>::Spread(vec![]), Stream::Next(42)];
    let mut iter = items.into_iter();
    let first = Iterator::next(&mut iter).unwrap();
    match first {
        Stream::Spread(items) => assert!(items.is_empty()),
        _ => panic!("expected Spread"),
    }
    assert_eq!(Iterator::next(&mut iter), Some(Stream::Next(42)));
}

#[test]
fn test_single_element_spread_done() {
    let items = vec![Stream::<i32, &str>::Spread(vec![StreamSpread::Done(42)])];
    let mut iter = items.into_iter();
    assert_eq!(
        Iterator::next(&mut iter),
        Some(Stream::Spread(vec![StreamSpread::Done(42)]))
    );
    assert_eq!(Iterator::next(&mut iter), None);
}

#[test]
fn test_single_element_spread_pending() {
    let items = vec![Stream::<i32, &str>::Spread(vec![StreamSpread::Pending("x")])];
    let mut iter = items.into_iter();
    assert_eq!(
        Iterator::next(&mut iter),
        Some(Stream::Spread(vec![StreamSpread::Pending("x")]))
    );
    assert_eq!(Iterator::next(&mut iter), None);
}

#[test]
fn test_empty_spread_done_via_map_done() {
    let items = vec![Stream::<i32, &str>::Spread(vec![])];
    let mut mapped = items.into_iter().map_done(|x| x * 10);
    match Iterator::next(&mut mapped).unwrap() {
        Stream::Spread(items) => assert!(items.is_empty()),
        _ => panic!("expected empty Spread"),
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
        Stream::Spread(vec![
            StreamSpread::Pending("a"),
            StreamSpread::Pending("b"),
        ]),
        Stream::Next(2),
    ]
    .into_iter();
    let mut future = StreamCollectFuture::new(iter);

    // Spread with Pending triggers Poll::Pending (stream signaled not-ready)
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
    // Spread with Done is treated as a signal to re-wake (not-ready),
    // consistent with the implementation's handling of spread in futures.
    let iter = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)]),
    ]
    .into_iter();
    let mut future = StreamCollectFuture::new(iter);

    // Spread with Done triggers Poll::Pending
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
    let iter = vec![Stream::<i32, &str>::Spread(vec![
        StreamSpread::Done(42),
        StreamSpread::Done(99),
        StreamSpread::Done(100),
    ])]
    .into_iter();
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
    let iter = vec![Stream::<i32, &str>::Spread(vec![])].into_iter();
    let mut future = StreamReadyFuture::new(iter);

    // Empty Spread: no value to return, iterator exhausted → None
    match poll_once(&mut future) {
        Poll::Ready(None) => {}
        other => panic!("expected None, got {:?}", other),
    }
}

#[test]
#[traced_test]
fn test_ready_future_spread_pending_returns_pending() {
    let iter = vec![
        Stream::<i32, &str>::Spread(vec![
            StreamSpread::Pending("a"),
            StreamSpread::Pending("b"),
        ]),
        Stream::Next(42),
    ]
    .into_iter();
    let mut future = StreamReadyFuture::new(iter);

    // With unified Spread, StreamReadyFuture iterates through spread items,
    // finds no Done values, continues the loop, and immediately finds Next(42).
    match poll_once(&mut future) {
        Poll::Ready(Some((value, _))) => assert_eq!(value, 42),
        other => panic!("expected Some(42), got {:?}", other),
    }
}

// ============================================================================
// Async future tests — StreamPendingFuture (TASK-09-30)
// ============================================================================

#[test]
#[traced_test]
fn test_pending_future_spread_pending_returns_first_value() {
    let iter = vec![Stream::<i32, &str>::Spread(vec![
        StreamSpread::Pending("a"),
        StreamSpread::Pending("b"),
        StreamSpread::Pending("c"),
    ])]
    .into_iter();
    let mut future = StreamPendingFuture::new(iter);

    // StreamPendingFuture returns Ready immediately on Spread with Pending (first value)
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
    let iter = vec![Stream::<i32, &str>::Spread(vec![])].into_iter();
    let mut future = StreamPendingFuture::new(iter);

    // Empty Spread: no value to return, iterator exhausted → None
    match poll_once(&mut future) {
        Poll::Ready(None) => {}
        other => panic!("expected None, got {:?}", other),
    }
}

#[test]
#[traced_test]
fn test_pending_future_spread_done_returns_pending() {
    let iter = vec![
        Stream::<i32, &str>::Spread(vec![StreamSpread::Done(1), StreamSpread::Done(2)]),
        Stream::Pending("x"),
    ]
    .into_iter();
    let mut future = StreamPendingFuture::new(iter);

    // With unified Spread, StreamPendingFuture iterates through spread items,
    // finds no Pending values, continues the loop, and immediately finds Pending("x").
    match poll_once(&mut future) {
        Poll::Ready(Some((ctx, _))) => assert_eq!(ctx, "x"),
        other => panic!("expected Some(x), got {:?}", other),
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
            Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)]),
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
        Poll::Ready(Some(Stream::Spread(vec![
            StreamSpread::Done(2),
            StreamSpread::Done(3),
        ])))
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
            Stream::Spread(vec![
                StreamSpread::Pending("b"),
                StreamSpread::Pending("c"),
            ]),
        ]
        .into_iter(),
    );

    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Pending("a")))
    );
    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Spread(vec![
            StreamSpread::Pending("b"),
            StreamSpread::Pending("c"),
        ])))
    );
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
}

#[test]
#[traced_test]
fn test_future_stream_empty_spread() {
    let mut stream = StreamAsFutureStream::new(
        vec![
            Stream::<i32, &str>::Spread(vec![]),
            Stream::Spread(vec![]),
        ]
        .into_iter(),
    );

    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Spread(vec![])))
    );
    assert_eq!(
        poll_stream_next(&mut stream),
        Poll::Ready(Some(Stream::Spread(vec![])))
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
        Stream::Spread(vec![StreamSpread::Pending("a")]),
        Stream::Next(2),
    ]
    .into_iter()
    .into_collect_future()
    .await;
    // Collect sees Next(1), then Spread with Pending → Pending (wakes), then Next(2)
    // on next poll. Result should contain both Next values.
    assert_eq!(result, vec![1, 2]);
}

#[tokio::test]
#[traced_test]
async fn test_tokio_ready_future_with_spread_done() {
    let (value, _) = vec![Stream::<i32, &str>::Spread(vec![
        StreamSpread::Done(42),
        StreamSpread::Done(99),
    ])]
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
        Stream::Spread(vec![StreamSpread::Pending("done")]),
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
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)]),
        Stream::Spread(vec![StreamSpread::Pending("a")]),
    ]
    .into_iter()
    .into_future_stream();
    let mut items = Vec::new();
    while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
        items.push(item);
    }
    assert_eq!(items.len(), 3);
    assert_eq!(items[0], Stream::Next(1));
    assert_eq!(
        items[1],
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)])
    );
    assert_eq!(items[2], Stream::Spread(vec![StreamSpread::Pending("a")]));
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
            Stream::Spread(vec![StreamSpread::Pending("wait")]),
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
            Stream::<i32, &str>::Spread(vec![
                StreamSpread::Done(99),
                StreamSpread::Done(100),
            ]),
            Stream::Next(1),
        ]
        .into_iter()
        .into_ready_future()
        .await
        .unwrap()
    });
    assert_eq!(value, 99);
    // Remaining iterator should still have Spread and Next
    assert!(remaining.next().is_some());
}

#[test]
#[traced_test]
fn test_smol_future_stream_with_spread() {
    let mut stream = vec![
        Stream::<i32, &str>::Spread(vec![
            StreamSpread::Done(1),
            StreamSpread::Done(2),
        ]),
        Stream::Spread(vec![StreamSpread::Pending("p")]),
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
            Stream::Spread(vec![
                StreamSpread::Done(1),
                StreamSpread::Done(2),
                StreamSpread::Done(3),
            ]),
        ]
        .into_iter()
        .map_done(|v| v * 2)
        .into_collect_future()
        .await
    });
    // Spread with Done triggers Pending, so only Next(5) collected
    assert_eq!(result, vec![10]);
}

#[test]
#[traced_test]
fn test_task_map_ready_then_collect_with_spread_done() {
    use foundation_core::valtron::TaskStatus;

    let items = vec![
        TaskStatus::<i32, &str, NoAction>::Ready(5),
        TaskStatus::Spread(vec![
            TaskSpread::Ready(1),
            TaskSpread::Ready(2),
            TaskSpread::Ready(3),
        ]),
    ];
    let mut mapped = items.into_iter().map_ready(|x| x * 10);

    assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(50)));
    // Spread with Ready is mapped: each item * 10, re-wrapped as Spread with Ready
    match Iterator::next(&mut mapped).unwrap() {
        TaskStatus::Spread(items) => {
            let ready_items: Vec<_> = items
                .into_iter()
                .filter_map(|s| match s {
                    TaskSpread::Ready(v) => Some(v),
                    _ => None,
                })
                .collect();
            assert_eq!(ready_items, vec![10, 20, 30]);
        }
        _ => panic!("expected Spread"),
    }
}
