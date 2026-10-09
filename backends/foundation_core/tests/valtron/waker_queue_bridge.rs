//! Feature 01 (Decision 00-F1): Waker → `QueueReadiness` bridge.
//!
//! These tests verify that a bridged `Future` parks via
//! `TaskStatus::Depends(QueueReadiness)` instead of busy-polling with a no-op
//! waker, and that a `wake()` — including one racing the park — is never lost.
//!
//! The contract-level tests exercise the exact `TaskIterator` ↔ executor
//! contract the scheduler consumes (`Depends` emitted on `Pending`; the signal
//! flips ready only when a `WakeToken` lands). They compile and run under both
//! the single/wasm `FutureTask` impl (default features) and the `multi` impl
//! (`--features multi`), covering both executors. The pool test drives a real
//! multi-threaded executor end-to-end.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{
    from_future, queue_waker, FutureTask, TaskIterator, TaskStatus, WakeToken,
};

// ============================================================================
// queue_waker mechanics
// ============================================================================

/// WHY: `wake()` must translate into "queue non-empty" so `QueueReadiness` fires.
/// WHAT: Both `wake_by_ref` and a cloned waker's `wake` push `WakeToken { id }`.
/// HOW: Build a waker over a fresh queue, fire it, and inspect the queue.
#[test]
fn queue_waker_wake_pushes_token_with_id() {
    let queue = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let waker = queue_waker(queue.clone(), 7);

    assert!(queue.is_empty(), "no wake yet -> queue empty");

    waker.wake_by_ref();
    assert_eq!(queue.pop(), Ok(WakeToken { id: 7 }), "wake_by_ref pushes token");
    assert!(queue.is_empty());

    // A cloned waker shares the same queue and id (RawWaker clone bumps the Arc).
    let cloned = waker.clone();
    cloned.wake(); // consumes the clone
    assert_eq!(queue.pop(), Ok(WakeToken { id: 7 }), "cloned wake pushes token");
    assert!(queue.is_empty());
}

/// WHY: Dropping wakers must not leak or double-free the backing `Arc`.
/// WHAT: Creating and dropping many clones leaves the queue usable.
/// HOW: Clone/drop in a loop, then confirm a final wake still works.
#[test]
fn queue_waker_clone_drop_is_sound() {
    let queue = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let waker = queue_waker(queue.clone(), 0);

    for _ in 0..10_000 {
        let c = waker.clone();
        drop(c);
    }

    waker.wake_by_ref();
    assert_eq!(queue.pop(), Ok(WakeToken { id: 0 }));
}

// ============================================================================
// Test future: parks until an external `ready` flag + a `wake()` arrive.
// ============================================================================

/// A future that stashes its waker and stays `Pending` until `ready` is set.
struct WakeParkFuture {
    poll_count: Arc<AtomicUsize>,
    ready: Arc<AtomicBool>,
    waker_slot: Arc<Mutex<Option<Waker>>>,
}

impl Future for WakeParkFuture {
    type Output = u64;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_count.fetch_add(1, Ordering::SeqCst);
        if self.ready.load(Ordering::SeqCst) {
            Poll::Ready(99)
        } else {
            *self.waker_slot.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// WHY: A `Pending` future must park (flat turn count), then complete once woken.
/// WHAT: First `next_status` yields `Depends` over a not-ready signal and polls
///       exactly once; the signal stays not-ready across many checks (the
///       executor's park predicate); after `wake()` it flips ready and the next
///       poll completes — total two polls, never a busy re-poll.
/// HOW: Drive `FutureTask::next_status` directly (the executor consumes exactly
///      this) and inspect the returned readiness signal.
#[test]
fn pending_future_parks_then_completes_on_wake() {
    let poll_count = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(AtomicBool::new(false));
    let waker_slot: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));

    let mut task = FutureTask::new(WakeParkFuture {
        poll_count: poll_count.clone(),
        ready: ready.clone(),
        waker_slot: waker_slot.clone(),
    });

    let signal = match task.next_status() {
        Some(TaskStatus::Depends(signal)) => signal,
        other => panic!("expected Depends on first poll, got {other:?}"),
    };
    assert_eq!(poll_count.load(Ordering::SeqCst), 1, "polled once, then parks");

    // Park predicate stays false while blocked — the executor would NOT re-run
    // the task, so the turn count is flat (no busy-poll).
    for _ in 0..1000 {
        assert!(!signal.is_ready(None), "parked signal must remain not-ready");
    }
    assert_eq!(poll_count.load(Ordering::SeqCst), 1, "no re-poll while parked");

    // Wake: set the result and fire the stashed waker — the signal flips ready.
    ready.store(true, Ordering::SeqCst);
    waker_slot.lock().unwrap().take().expect("waker stashed").wake();
    assert!(signal.is_ready(None), "wake() makes the signal ready to re-run");

    // The unparked re-run drains the stale token, re-polls, and completes.
    match task.next_status() {
        Some(TaskStatus::Ready(99)) => {}
        other => panic!("expected Ready(99) after wake, got {other:?}"),
    }
    assert_eq!(poll_count.load(Ordering::SeqCst), 2, "exactly two polls total");
    assert!(task.next_status().is_none(), "task is complete");
}

/// A future that wakes itself during its first poll, then completes on the next.
///
/// This reproduces the self-wake pattern used by stream-future bridges
/// (e.g. `StreamReadyFuture`): the future calls `cx.waker().wake_by_ref()` during
/// `poll()` and returns `Pending`. `FutureTask` detects the self-wake token and
/// returns `Pending` (requeue) instead of `Depends` — the executor re-runs the task
/// without ever parking. The wakeup is never lost (Decision 00 success criterion).
struct SelfWakeOnceFuture {
    polled: bool,
}

impl Future for SelfWakeOnceFuture {
    type Output = u64;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.polled {
            Poll::Ready(1)
        } else {
            self.polled = true;
            cx.waker().wake_by_ref(); // self-wake: "re-poll me"
            Poll::Pending
        }
    }
}

/// WHY: A `wake()` produced *during* poll must not be lost (Decision 00 success
///      criterion). The self-wake token triggers `Pending` (requeue) so the
///      executor re-runs the task and completes without parking.
/// WHAT: A future that self-wakes on its first poll completes in exactly two polls
///       — no park, no `Depends-true`, no lost wakeup.
/// HOW: Stress 2000 iterations; each fresh task must complete on the second poll.
#[test]
fn wake_before_park_is_never_lost() {
    use foundation_core::valtron::FuturePollState;

    for iteration in 0..2000 {
        let mut task = FutureTask::new(SelfWakeOnceFuture { polled: false });

        // First poll: future self-wakes → Pending (requeue), not Depends.
        match task.next_status() {
            Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
            other => panic!("iter {iteration}: expected Pending(FuturePollState::Pending), got {other:?}"),
        };

        // Second poll: future returns Ready.
        match task.next_status() {
            Some(TaskStatus::Ready(1)) => {}
            other => panic!("iter {iteration}: expected Ready(1), got {other:?}"),
        };

        assert!(task.next_status().is_none(), "iter {iteration}: complete");
    }
}

// ============================================================================
// End-to-end on the real multi-threaded executor
// ============================================================================

/// WHY: Prove the parking path works through a real executor, not just the
///      `TaskIterator` contract — a parked future is unparked by a `wake()` from
///      another thread and completes, without busy-polling.
/// WHAT: Spawn a bridged future on the pool; a helper thread wakes it once it has
///       parked; the future completes and was polled only a handful of times.
/// HOW: `from_future` + `schedule_iter`; wake from a side thread after the waker
///      is stashed.
#[cfg(feature = "multi")]
#[cfg_attr(feature = "multi", tracing_test::traced_test)]
#[foundation_core::valtron::valtron_test(threads = 2)]
fn future_parks_and_wakes_on_pool() {
    use foundation_core::valtron::{get_pool, NotificationItem};
    use std::thread;
    use std::time::Duration;

    let poll_count = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(AtomicBool::new(false));
    let waker_slot: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));

    let fut = WakeParkFuture {
        poll_count: poll_count.clone(),
        ready: ready.clone(),
        waker_slot: waker_slot.clone(),
    };

    // Wake the task once it has parked (stashed its waker) on the pool.
    let ws = waker_slot.clone();
    let rd = ready.clone();
    let waker_thread = thread::spawn(move || {
        loop {
            let maybe_waker = ws.lock().unwrap().clone();
            if let Some(w) = maybe_waker {
                rd.store(true, Ordering::SeqCst);
                w.wake();
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
    });

    let iter = get_pool()
        .spawn()
        .with_task(from_future(fut))
        .schedule_iter(Duration::from_millis(1))
        .expect("should schedule task");

    let mut result = None;
    for item in iter {
        if let NotificationItem::Ready(TaskStatus::Ready(value)) = item {
            result = Some(value);
            break;
        }
    }

    waker_thread.join().expect("waker thread finishes");

    assert_eq!(result, Some(99), "future completes after being woken");
    // Parked, not spun: one poll to park + one poll after the wake.
    assert_eq!(
        poll_count.load(Ordering::SeqCst),
        2,
        "future parked (no busy-poll): exactly two polls"
    );
}
