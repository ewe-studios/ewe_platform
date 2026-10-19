//! Regression tests for `Stream::Spread` draining across every Done-collector.
//!
//! `Stream::Spread(Vec<StreamSpread<D, P>>)` carries multiple `Done(D)` /
//! `Pending(P)` payloads in a single iterator item. Every collector whose job is
//! to surface `Done` values must **drain** every `Done` payload (and skip the
//! `Pending` ones), never drop them. This pins:
//!
//! - `StreamCollectFuture` / `into_collect_future()` — the collect bridge;
//! - `collect_result` / `collect_one` — the sync boundary helpers;
//! - `StreamNextStream` / `into_next_stream()` — the awaitable value stream;
//! - the register-then-recheck arm of `StreamCollectFuture` (the lost-wakeup
//!   path, where a `Spread` can arrive *between* `next()` and `wake()`).
//!
//! Cross-runtime coverage (tokio, smol, and valtron's own `#[valtron_test]`
//! executor) validates that the bridges drain identically under each runtime.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use futures_core::Stream as FuturesStream;

use foundation_core::valtron::{
    collect_one, collect_result, execute, valtron_test, Ctx, Stream, StreamCollectFuture,
    StreamIteratorExt, StreamSpread, TaskIterator, TaskSpread, TaskStatus, WakeCallback, WakeResult,
    WakeableStream,
};

// ============================================================================
// No-op waker for sync polling
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

/// Drives a `futures_core::Stream` to completion inside an async runtime by
/// polling it with the runtime's own waker (via `poll_fn`). This exercises the
/// runtime's poller and parking, not just a manual no-op poll.
async fn drain_stream<S>(mut stream: S) -> Vec<S::Item>
where
    S: FuturesStream + Unpin,
{
    use core::future::poll_fn;

    let mut out = Vec::new();
    poll_fn(|cx| loop {
        match Pin::new(&mut stream).poll_next(cx) {
            Poll::Ready(Some(item)) => out.push(item),
            Poll::Ready(None) => return Poll::Ready(()),
            Poll::Pending => return Poll::Pending,
        }
    })
    .await;
    out
}

// ============================================================================
// StreamCollectFuture — top-level Spread draining
// ============================================================================

#[test]
fn collect_future_drains_spread_done() {
    let iter = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)]),
        Stream::Next(4),
    ]
    .into_iter()
    .into_stream_iter();
    let mut future = StreamCollectFuture::new(iter);

    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![1, 2, 3, 4]),
        Poll::Pending => panic!("expected Ready"),
    }
}

#[test]
fn collect_future_mixed_spread_skips_pending_drains_done() {
    let iter = vec![
        Stream::<i32, &str>::Spread(vec![
            StreamSpread::Pending("a"),
            StreamSpread::Done(1),
            StreamSpread::Pending("b"),
            StreamSpread::Done(2),
        ]),
        Stream::Next(3),
    ]
    .into_iter()
    .into_stream_iter();
    let mut future = StreamCollectFuture::new(iter);

    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![1, 2, 3]),
        Poll::Pending => panic!("expected Ready"),
    }
}

#[test]
fn collect_future_spread_only_pending_is_skipped() {
    let iter = vec![
        Stream::<i32, &str>::Spread(vec![StreamSpread::Pending("a"), StreamSpread::Pending("b")]),
        Stream::Next(9),
    ]
    .into_iter()
    .into_stream_iter();
    let mut future = StreamCollectFuture::new(iter);

    // No Done payloads in the Spread → the bridge keeps pulling and collects Next(9).
    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![9]),
        Poll::Pending => panic!("expected Ready"),
    }
}

#[test]
fn collect_future_empty_spread_continues() {
    let iter = vec![
        Stream::<i32, &str>::Spread(vec![]),
        Stream::Spread(vec![]),
        Stream::Next(42),
    ]
    .into_iter()
    .into_stream_iter();
    let mut future = StreamCollectFuture::new(iter);

    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![42]),
        Poll::Pending => panic!("expected Ready"),
    }
}

// ============================================================================
// StreamCollectFuture — register-then-recheck arm Spread draining
// ============================================================================

/// Scripted iterator that yields `Wait` on its first poll (so the bridge enters
/// the wake arm), then a `Spread` on the re-check `next()` (simulating a push
/// landing between the bridge's `next()` and `wake()`), then `None`.
struct RecheckSpreadIter {
    step: u8,
}

impl Iterator for RecheckSpreadIter {
    type Item = Stream<i32, &'static str>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.step {
            0 => {
                self.step = 1;
                Some(Stream::Wait)
            }
            1 => {
                self.step = 2;
                Some(Stream::Spread(vec![StreamSpread::Done(1), StreamSpread::Done(2)]))
            }
            _ => None,
        }
    }
}

impl WakeableStream for RecheckSpreadIter {
    fn wake(&mut self, _cb: WakeCallback) -> WakeResult {
        // The callback is intentionally dropped: this scripted iterator resolves
        // on the re-check, so the bridge never actually parks.
        WakeResult::Successful
    }
}

#[test]
fn collect_future_recheck_drains_spread_done() {
    let mut future = StreamCollectFuture::new(RecheckSpreadIter { step: 0 });

    // The first poll hits `Wait`, registers (Successful), re-checks and finds the
    // `Spread`, drains both Done payloads, then exhausts — all in one poll.
    match poll_once(&mut future) {
        Poll::Ready(v) => assert_eq!(v, vec![1, 2]),
        other => panic!("expected Ready(vec![1, 2]), got {other:?}"),
    }
}

// ============================================================================
// Sync boundary helpers — collect_result / collect_one
// ============================================================================

#[test]
fn collect_result_drains_spread_done() {
    let stream: Vec<Stream<u32, ()>> = vec![
        Stream::Next(1),
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)]),
        Stream::Next(4),
    ];
    assert_eq!(collect_result(stream.into_iter()), vec![1, 2, 3, 4]);
}

#[test]
fn collect_result_skips_spread_pending() {
    let stream: Vec<Stream<u32, &str>> = vec![Stream::Spread(vec![
        StreamSpread::Pending("a"),
        StreamSpread::Done(5),
        StreamSpread::Pending("b"),
    ])];
    assert_eq!(collect_result(stream.into_iter()), vec![5]);
}

#[test]
fn collect_result_spread_only_pending_collects_nothing() {
    let stream: Vec<Stream<u32, &str>> = vec![Stream::Spread(vec![
        StreamSpread::Pending("a"),
        StreamSpread::Pending("b"),
    ])];
    assert!(collect_result(stream.into_iter()).is_empty());
}

#[test]
fn collect_one_finds_done_in_spread() {
    let stream: Vec<Stream<u32, &str>> = vec![Stream::Spread(vec![
        StreamSpread::Pending("a"),
        StreamSpread::Done(42),
        StreamSpread::Done(99),
    ])];
    assert_eq!(collect_one(stream.into_iter()), Some(42));
}

#[test]
fn collect_one_skips_spread_with_only_pending() {
    let stream: Vec<Stream<u32, &str>> = vec![
        Stream::Spread(vec![StreamSpread::Pending("a")]),
        Stream::Next(7),
    ];
    assert_eq!(collect_one(stream.into_iter()), Some(7));
}

// ============================================================================
// StreamNextStream — lossless Spread fan-out
// ============================================================================

#[test]
fn next_stream_fans_out_spread_done() {
    let mut stream = vec![Stream::<i32, &str>::Spread(vec![
        StreamSpread::Done(1),
        StreamSpread::Done(2),
        StreamSpread::Done(3),
    ])]
    .into_iter()
    .into_stream_iter()
    .into_next_stream();

    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(1)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(2)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(3)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
}

#[test]
fn next_stream_mixed_spread_and_next() {
    let mut stream = vec![
        Stream::<i32, &str>::Next(0),
        Stream::Spread(vec![StreamSpread::Done(1), StreamSpread::Done(2)]),
        Stream::Next(3),
    ]
    .into_iter()
    .into_stream_iter()
    .into_next_stream();

    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(0)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(1)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(2)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(3)));
    assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
}

// ============================================================================
// Cross-runtime — tokio
// ============================================================================

#[tokio::test]
async fn tokio_collect_future_drains_spread_done() {
    let result = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Done(3)]),
    ]
    .into_iter()
    .into_stream_iter()
    .into_collect_future()
    .await;
    assert_eq!(result, vec![1, 2, 3]);
}

#[tokio::test]
async fn tokio_next_stream_fans_out_spread() {
    let stream = vec![
        Stream::<i32, &str>::Spread(vec![StreamSpread::Done(10), StreamSpread::Done(20)]),
        Stream::Next(30),
    ]
    .into_iter()
    .into_stream_iter()
    .into_next_stream();

    assert_eq!(drain_stream(stream).await, vec![10, 20, 30]);
}

// ============================================================================
// Cross-runtime — smol
// ============================================================================

#[test]
fn smol_collect_future_drains_spread_done() {
    let result = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Spread(vec![
                StreamSpread::Pending("wait"),
                StreamSpread::Done(1),
                StreamSpread::Done(2),
            ]),
            Stream::Next(3),
        ]
        .into_iter()
        .into_stream_iter()
        .into_collect_future()
        .await
    });
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn smol_next_stream_fans_out_spread() {
    let stream = vec![
        Stream::<i32, &str>::Spread(vec![StreamSpread::Done(5), StreamSpread::Done(6)]),
        Stream::Spread(vec![StreamSpread::Pending("x")]),
        Stream::Next(7),
    ]
    .into_iter()
    .into_stream_iter()
    .into_next_stream();

    assert_eq!(smol::block_on(drain_stream(stream)), vec![5, 6, 7]);
}

// ============================================================================
// Async task wrapped by a valtron future (execute → into_collect_future)
// ============================================================================

/// A task that emits a single `TaskStatus::Spread` of `Ready` values. The
/// executor converts it to `Stream::Spread(Done(..))`, which the collect bridge
/// must drain losslessly.
struct SpreadTask {
    emitted: bool,
}

impl Default for SpreadTask {
    fn default() -> Self {
        Self { emitted: false }
    }
}

impl TaskIterator for SpreadTask {
    type Ready = i32;
    type Pending = &'static str;

    fn next_status(
        &mut self,
        _ctx: Ctx,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.emitted {
            None
        } else {
            self.emitted = true;
            Some(TaskStatus::Spread(vec![
                TaskSpread::Ready(1),
                TaskSpread::Ready(2),
                TaskSpread::Ready(3),
            ]))
        }
    }
}

/// Full chain: a valtron task → `execute` → stream → `into_collect_future`, awaited
/// under valtron's own executor. Every `Spread` `Done` value must be collected.
#[valtron_test(seed = 42)]
async fn valtron_execute_spread_task_collects_all_done() {
    let result = execute(SpreadTask::default(), None)
        .expect("execute should succeed")
        .into_collect_future()
        .await;
    assert_eq!(result, vec![1, 2, 3]);
}
