//! Feature 03: bridge parking over the `WakeableStream` wake side-channel.
//!
//! These tests pin the four bridges that actually self-wake today
//! (`StreamCollectFuture`, `StreamReadyFuture`, `StreamPendingFuture`,
//! `StreamNextStream`). Each is driven over a `NotifyQueueStreamIterator` (the
//! waking terminal from feature 02) with a counting waker, so a fired callback is
//! *observable*: the waker count proves whether the bridge parked (registered a
//! callback and returned `Poll::Pending` without self-waking) or spun.
//!
//! `StreamAsFutureStream` is intentionally absent: it is a raw lens whose
//! `poll_next` always returns `Poll::Ready(iter.next())`, so it never parks and
//! carries no `WakeableStream` bound.
//!
//! `NotifyQueue::set_max_spins(1)` makes an empty `next()` return `Stream::Wait`
//! after a single short CondVar wait, keeping the tests single-threaded,
//! fast, and deterministic (no producer thread is needed — the test itself
//! pushes/closes between polls).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Wake;
use std::time::Duration;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use futures_core::Stream as FuturesStream;

use foundation_core::valtron::{
    NotifyQueue, NotifyQueueStreamIterator, Stream, StreamCollectFuture, StreamIteratorExt,
    StreamNextStream, StreamPendingFuture, StreamReadyFuture,
};

// ============================================================================
// Counting waker — proves park-vs-spin by counting every `wake`/`wake_by_ref`.
// ============================================================================

/// A `Wake` implementation whose shared counter records every wake. `wake_by_ref`
/// (the default) clones the `Arc` and calls `wake`, so both paths increment.
struct CountingWaker {
    count: Arc<AtomicUsize>,
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

/// Builds a `Waker` backed by a shared counter, returning both.
fn counting_waker() -> (Waker, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(CountingWaker {
        count: Arc::clone(&count),
    }));
    (waker, count)
}

fn poll<F: Future + Unpin>(f: &mut F, waker: &Waker) -> Poll<F::Output> {
    let mut cx = Context::from_waker(waker);
    Pin::new(f).poll(&mut cx)
}

fn poll_next<S: FuturesStream + Unpin>(s: &mut S, waker: &Waker) -> Poll<Option<S::Item>> {
    let mut cx = Context::from_waker(waker);
    Pin::new(s).poll_next(&mut cx)
}

/// Builds a waking `NotifyQueueStreamIterator` whose empty `next()` yields
/// `Stream::Wait` after one short CondVar wait, returning the iterator and a
/// producer handle (a clone of the same underlying queue) for `push`/`close`.
fn waking_iter<D, P>() -> (NotifyQueueStreamIterator<D, P>, Arc<NotifyQueue<Stream<D, P>>>) {
    let queue: Arc<NotifyQueue<Stream<D, P>>> = Arc::new(NotifyQueue::unbounded());
    queue.set_max_spins(1);
    let iterator = NotifyQueueStreamIterator::new(Arc::clone(&queue), 5, Duration::from_micros(50));
    (iterator, queue)
}

// ============================================================================
// (a) A producer `push` wakes a parked bridge (no self-wake spin)
// ============================================================================

/// WHY: On an empty queue the bridge must PARK — register a wake and return
///      `Pending` with no self-wake — and only resolve when the producer pushes.
/// WHAT: First poll on an empty `NotifyQueueStreamIterator` returns `Pending`
///       with a zero wake count; `push` fires the callback; the re-poll resolves.
#[test]
fn ready_future_parks_then_wakes_on_push() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamReadyFuture::new(iterator);
    let (waker, count) = counting_waker();

    // Empty queue → `Wait` → registers wake, re-checks (still empty), parks.
    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    // Producer push fires the registered callback → the waker wakes exactly once.
    queue.push(Stream::Next(42)).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1, "push fired the wake");

    // Re-poll observes the item and resolves with the remaining iterator.
    match poll(&mut future, &waker) {
        Poll::Ready(Some((42, _))) => {}
        other => panic!("expected Ready(Some((42, _))), got {other:?}"),
    }
}

/// Same parking contract for the other Done-collector: `StreamCollectFuture`.
#[test]
fn collect_future_parks_then_wakes_on_push() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamCollectFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    queue.push(Stream::Next(7)).unwrap();
    queue.push(Stream::Next(8)).unwrap();
    // Both pushes fire the (single) registered callback — the registry fires then
    // clears, so the second `push` sees an empty registry and fires nothing.
    assert_eq!(count.load(Ordering::SeqCst), 1, "only the first push fires the registered wake");

    // Re-poll drains both items, then parks again on the empty queue.
    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 1, "no self-wake while parked");

    // Close resolves the parked collect to its accumulated vec.
    queue.close();
    match poll(&mut future, &waker) {
        Poll::Ready(vec) => assert_eq!(vec, vec![7, 8], "collected both pushed items"),
        other => panic!("expected Ready(vec![7, 8]) on close, got {other:?}"),
    }
}

/// `StreamNextStream` is the awaitable `Stream` bridge (`Item = SI::D`); it parks
/// on `Wait` and surfaces the first `Next` payload once pushed.
#[test]
fn next_stream_parks_then_wakes_on_push() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut stream = StreamNextStream::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_next(&mut stream, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    queue.push(Stream::Next(5)).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1, "push fired the wake");

    match poll_next(&mut stream, &waker) {
        Poll::Ready(Some(5)) => {}
        other => panic!("expected Ready(Some(5)), got {other:?}"),
    }
}

// ============================================================================
// (b) `close` wakes a parked bridge to `None`
// ============================================================================

/// WHY: A producer closing the queue is itself a readiness edge — the parked
///      bridge must be woken to observe exhaustion and return `None` (or the
///      accumulated vec), not hang parked forever.
/// WHAT: Park on empty, `close`, then the next poll resolves to `Ready(None)`.
#[test]
fn ready_future_close_wakes_to_none() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamReadyFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    assert!(queue.close());
    assert_eq!(count.load(Ordering::SeqCst), 1, "close fired the wake");

    match poll(&mut future, &waker) {
        Poll::Ready(None) => {}
        other => panic!("expected Ready(None) on close, got {other:?}"),
    }
}

// ============================================================================
// (c) A `NotSupported` iterator still resolves via self-wake
// ============================================================================

/// WHY: `into_stream_iter()` wraps a plain iterator in `StreamIter`, which reports
///      `WakeResult::NotSupported`; the bridge must fall back to the legacy
///      `cx.waker().wake_by_ref()` self-wake so an in-memory `Vec<Stream>` still
///      resolves instead of parking forever.
/// WHAT: Poll past `Init` self-wakes (wake count 1), then the next poll reaches
///       `Next(7)` and resolves.
#[test]
fn ready_future_not_supported_self_wakes() {
    let iter = vec![Stream::<i32, &'static str>::Init, Stream::Next(7)]
        .into_iter()
        .into_stream_iter();
    let mut future = StreamReadyFuture::new(iter);
    let (waker, count) = counting_waker();

    // `Init` is non-target → NotSupported → self-wake.
    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 1, "NotSupported self-wakes");

    match poll(&mut future, &waker) {
        Poll::Ready(Some((7, _))) => {}
        other => panic!("expected Ready(Some((7, _))), got {other:?}"),
    }
}

// ============================================================================
// (d) A spurious wake re-parks and re-registers correctly
// ============================================================================

/// WHY: The wake signal carries no payload, so a producer `push` of a *wrong*
///      variant (a `Next` for `StreamPendingFuture`, whose target is `Pending`)
///      still fires the callback. The bridge must consume the spurious item,
///      re-park, and re-register — not resolve on the wrong item and not spin.
/// WHAT: Park on empty; push a `Next` (spurious) and re-poll → still `Pending`
///       with no self-wake; push the real `Pending` → the re-registered callback
///       fires and the bridge resolves.
#[test]
fn pending_future_spurious_wake_reparks_and_reregisters() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamPendingFuture::new(iterator);
    let (waker, count) = counting_waker();

    // Park on the empty queue.
    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    // Spurious wake: `Next` is not the `Pending` target.
    queue.push(Stream::Next(99)).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1, "spurious push fired the wake");

    // Re-poll consumes the spurious Next, re-parks, and re-registers — no self-wake.
    assert!(matches!(poll(&mut future, &waker), Poll::Pending));
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "spurious item re-parks without self-wake"
    );

    // The real target fires the re-registered callback.
    queue.push(Stream::Pending("ctx")).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "re-registered callback fired");

    match poll(&mut future, &waker) {
        Poll::Ready(Some(("ctx", _))) => {}
        other => panic!("expected Ready(Some((\"ctx\", _))), got {other:?}"),
    }
}
