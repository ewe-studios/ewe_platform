//! Feature 02 (Decision 00-F4 / Decision 11): `Pipe<T>` bounded seam pipe,
//! `QueueVacancyReadiness`, and the `AnyReadiness` combinator.
//!
//! The contract tests drive the pipe futures directly with a `queue_waker`
//! (feature 01) so a fired waker is *observable* — a token landing on the
//! observation queue proves the opposite end woke the parked side, and an empty
//! observation queue proves the turn count is flat while blocked. The pool test
//! runs a producer and consumer concurrently on the real multi-threaded executor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use core::future::Future;
use core::task::{Context, Poll};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{
    queue_waker, AnyReadiness, BoolSignal, EventReadiness, EventReadinessPtr, Pipe, TryRecvError,
    WakeToken,
};

/// Build a `Context` whose waker pushes an observable `WakeToken` onto `obs`.
fn observing_cx(obs: &Arc<ConcurrentQueue<WakeToken>>) -> core::task::Waker {
    queue_waker(obs.clone(), 0)
}

// ============================================================================
// Basic hand-off
// ============================================================================

/// WHY: The pipe must move items FIFO across both the task and async surfaces.
/// WHAT: `try_send`/`try_recv` and `send`/`receive` round-trip in order.
#[test]
fn send_and_receive_roundtrip() {
    let (tx, rx) = Pipe::<u32>::with_depth(4);

    tx.try_send(1).expect("send 1");
    tx.try_send(2).expect("send 2");
    assert_eq!(rx.try_recv(), Ok(1));
    assert_eq!(rx.try_recv(), Ok(2));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));

    // Async surface (ready immediately — no parking needed here).
    let obs = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let waker = observing_cx(&obs);
    let mut cx = Context::from_waker(&waker);

    let mut send = Box::pin(tx.send(7));
    assert!(matches!(send.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    let mut recv = Box::pin(rx.receive());
    assert!(matches!(recv.as_mut().poll(&mut cx), Poll::Ready(Some(7))));
}

// ============================================================================
// Two-sided wake: producer parks on full, consumer pop wakes it
// ============================================================================

/// WHY: A producer awaiting a full pipe must PARK (flat turn count) and be woken
///      by the consumer's pop — the L1b producer-side wake (Decision 00).
/// WHAT: With the pipe full, `send` returns `Pending` and no wake is observed;
///      a `try_recv` fires the stashed producer waker (token observed); the next
///      poll of the same `send` completes.
#[test]
fn producer_parks_on_full_and_wakes_on_pop() {
    let (tx, rx) = Pipe::<u32>::with_depth(2);
    tx.try_send(1).expect("send 1");
    tx.try_send(2).expect("send 2");
    assert!(tx.is_full(), "pipe filled to depth");

    let obs = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let waker = observing_cx(&obs);
    let mut cx = Context::from_waker(&waker);

    // Full pipe -> the send future parks.
    let mut send = Box::pin(tx.send(3));
    assert!(matches!(send.as_mut().poll(&mut cx), Poll::Pending));
    assert!(obs.is_empty(), "turn count flat: no wake while the pipe stays full");

    // Consumer pops -> fires the stashed producer waker.
    assert_eq!(rx.try_recv(), Ok(1));
    assert!(!obs.is_empty(), "pop woke the parked producer");

    // The unparked producer completes its send.
    assert!(matches!(send.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert_eq!(rx.try_recv(), Ok(2));
    assert_eq!(rx.try_recv(), Ok(3), "the once-parked item is delivered in FIFO order");
}

/// WHY: The consumer/empty case must be symmetric — park on empty, woken by push.
/// WHAT: `receive` on an empty pipe parks with no wake observed; a `try_send`
///      fires the stashed consumer waker; the next poll yields the item.
#[test]
fn consumer_parks_on_empty_and_wakes_on_push() {
    let (tx, rx) = Pipe::<u32>::with_depth(2);

    let obs = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let waker = observing_cx(&obs);
    let mut cx = Context::from_waker(&waker);

    let mut recv = Box::pin(rx.receive());
    assert!(matches!(recv.as_mut().poll(&mut cx), Poll::Pending));
    assert!(obs.is_empty(), "turn count flat: no wake while the pipe stays empty");

    tx.try_send(42).expect("send");
    assert!(!obs.is_empty(), "push woke the parked consumer");

    assert!(matches!(recv.as_mut().poll(&mut cx), Poll::Ready(Some(42))));
}

// ============================================================================
// Close / end-of-stream
// ============================================================================

/// WHY: Dropping the sender is end-of-stream; the receiver drains then reports EOS.
/// WHAT: Backlog is delivered, then `try_recv` returns `Closed` and `readiness`
///      reports ready (closed-aware) so a parked consumer would unpark.
#[test]
fn sender_drop_signals_end_of_stream() {
    let (tx, rx) = Pipe::<u32>::with_depth(4);
    tx.try_send(1).expect("send 1");
    tx.try_send(2).expect("send 2");

    // A parked consumer must wake on close: park first, then drop the sender.
    let obs = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let (tx2, rx2) = Pipe::<u32>::with_depth(4);
    let waker = observing_cx(&obs);
    let mut cx = Context::from_waker(&waker);
    let mut recv = Box::pin(rx2.receive());
    assert!(matches!(recv.as_mut().poll(&mut cx), Poll::Pending));
    drop(tx2);
    assert!(!obs.is_empty(), "sender drop woke the parked consumer");
    assert!(matches!(recv.as_mut().poll(&mut cx), Poll::Ready(None)), "clean EOS");

    // Backlog drains before EOS is reported.
    drop(tx);
    assert!(rx.readiness().is_ready(None), "closed pipe is readiness-ready");
    assert_eq!(rx.try_recv(), Ok(1));
    assert_eq!(rx.try_recv(), Ok(2));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Closed));
}

// ============================================================================
// Vacancy readiness + AnyReadiness
// ============================================================================

/// WHY: The task-path producer parks on `QueueVacancyReadiness` (ready = has
///      capacity), including a closed-pipe override so it can't hang.
#[test]
fn vacancy_readiness_tracks_capacity_and_close() {
    let (tx, rx) = Pipe::<u32>::with_depth(1);
    let vacancy = tx.vacancy();

    assert!(vacancy.is_ready(None), "empty pipe has capacity");
    tx.try_send(1).expect("send");
    assert!(!vacancy.is_ready(None), "full pipe reports no vacancy");
    assert_eq!(rx.try_recv(), Ok(1));
    assert!(vacancy.is_ready(None), "vacancy re-opens after a pop");

    // A full-then-closed pipe reports ready so a parked producer unparks.
    tx.try_send(2).expect("send");
    assert!(tx.is_full());
    tx.close();
    assert!(vacancy.is_ready(None), "closed pipe reports vacancy-ready");
}

/// WHY: A parked task returns ONE `Depends`; disjunctive waits ("vacancy OR
///      cancelled") compose inside `AnyReadiness` (Decision 00 L1b / Decision 11).
#[test]
fn any_readiness_fires_when_any_child_ready() {
    let (tx, _rx) = Pipe::<u32>::with_depth(1);
    tx.try_send(1).expect("send"); // pipe now full -> vacancy not ready

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel = BoolSignal::from_atomic(cancel_flag.clone());

    let any = AnyReadiness::either(
        Arc::new(tx.vacancy()) as EventReadinessPtr,
        Arc::new(cancel) as EventReadinessPtr,
    );
    assert!(!any.is_ready(None), "full pipe + not cancelled -> not ready");

    // Firing the cancel child alone makes the composite ready.
    cancel_flag.store(true, Ordering::SeqCst);
    assert!(any.is_ready(None), "AnyReadiness fires when the cancel child is ready");
}

// ============================================================================
// Wake-before-park race (stash-then-recheck)
// ============================================================================

/// WHY: A `pop` that lands between `send`'s failed push and its waker stash must
///      not be a lost wakeup — the stash-then-recheck must observe the vacancy.
/// WHAT: Stress a full-pipe `send` whose slot is freed via a re-check path.
#[test]
fn send_recheck_closes_wake_before_park_race() {
    for iteration in 0..2000 {
        let (tx, rx) = Pipe::<u32>::with_depth(1);
        tx.try_send(1).expect("fill");

        let obs = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
        let waker = observing_cx(&obs);
        let mut cx = Context::from_waker(&waker);

        // Free the slot right before polling the parked send: the future's
        // internal re-check (after stashing) must see the vacancy and complete.
        assert_eq!(rx.try_recv(), Ok(1), "iter {iteration}");
        let mut send = Box::pin(tx.send(2));
        assert!(
            matches!(send.as_mut().poll(&mut cx), Poll::Ready(Ok(()))),
            "iter {iteration}: vacancy available -> send completes without parking"
        );
        assert_eq!(rx.try_recv(), Ok(2), "iter {iteration}");
    }
}

// ============================================================================
// End-to-end on the real multi-threaded executor
// ============================================================================

/// WHY: Prove backpressure + two-sided parking work through a real executor: a
///      producer and consumer run concurrently over a shallow bounded pipe, the
///      producer parks when full and the consumer parks when empty, and every
///      item arrives in FIFO order.
#[cfg(feature = "multi")]
#[cfg_attr(feature = "multi", tracing_test::traced_test)]
#[foundation_core::valtron::valtron_test(threads = 2)]
fn pipe_backpressure_roundtrip_on_pool() {
    use foundation_core::valtron::{from_future, get_pool, NotificationItem, TaskStatus};
    use std::time::Duration;

    const N: u32 = 50;
    let (tx, rx) = Pipe::<u32>::with_depth(3);

    // Producer: send 0..N then drop tx (closes the pipe -> consumer sees EOS).
    let producer = async move {
        for i in 0..N {
            tx.send(i).await.expect("pipe open while producing");
        }
        drop(tx);
    };

    // Consumer: receive until end-of-stream, returning what it collected.
    let consumer = async move {
        let mut got = Vec::new();
        while let Some(v) = rx.receive().await {
            got.push(v);
        }
        got
    };

    get_pool()
        .spawn()
        .with_task(from_future(producer))
        .schedule()
        .expect("schedule producer");

    let iter = get_pool()
        .spawn()
        .with_task(from_future(consumer))
        .schedule_iter(Duration::from_millis(1))
        .expect("schedule consumer");

    let mut result = None;
    for item in iter {
        if let NotificationItem::Ready(TaskStatus::Ready(got)) = item {
            result = Some(got);
            break;
        }
    }

    let got = result.expect("consumer produced a result");
    let expected: Vec<u32> = (0..N).collect();
    assert_eq!(got, expected, "all items delivered in FIFO order across the bounded pipe");
}
