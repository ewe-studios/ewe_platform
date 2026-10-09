use foundation_core::valtron::{FutureTask, TaskStatus, from_future};
use foundation_core::valtron::TaskIterator;

/// WHY: `FutureTask` must poll and complete a simple future
/// WHAT: Future that returns Ready immediately should complete on first poll
#[test]
fn test_future_task_immediate_ready() {
    let future = core::future::ready(42);
    let mut task = FutureTask::new(future);

    // First poll should return Ready(42)
    match task.next_status() {
        Some(TaskStatus::Ready(42)) => {}
        other => panic!("Expected Ready(42), got {other:?}"),
    }

    // Task is complete
    assert!(task.next_status().is_none());
}

/// WHY: `FutureTask` must park (not busy-poll) when the inner future is pending
/// WHAT: A `Poll::Pending` yields `Depends(QueueReadiness)` so the executor parks;
///       a later poll (after the drain) still completes to Ready (Decision 00-F1)
#[test]
fn test_future_task_pending() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct PendingThenReady {
        polled: bool,
    }

    impl Future for PendingThenReady {
        type Output = i32;

        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.polled {
                Poll::Ready(100)
            } else {
                self.polled = true;
                Poll::Pending
            }
        }
    }

    let future = PendingThenReady { polled: false };
    let mut task = FutureTask::new(future);

    // First poll parks the task: Depends over an (empty) wake queue — the signal
    // is NOT ready, so the executor would sleep instead of re-polling.
    match task.next_status() {
        Some(TaskStatus::Depends(signal)) => {
            assert!(
                !signal.is_ready(None),
                "empty wake queue must report not-ready so the task actually parks"
            );
        }
        other => panic!("Expected Depends(signal), got {other:?}"),
    }

    // Second poll should be Ready
    match task.next_status() {
        Some(TaskStatus::Ready(100)) => {}
        other => panic!("Expected Ready(100), got {other:?}"),
    }

    // Task is complete
    assert!(task.next_status().is_none());
}

/// WHY: `from_future()` convenience function must work
/// WHAT: Creates `FutureTask` correctly
#[test]
fn test_from_future_convenience() {
    let future = core::future::ready("hello");
    let mut task = from_future(future);

    match task.next_status() {
        Some(TaskStatus::Ready("hello")) => {}
        other => panic!("Expected Ready(hello), got {other:?}"),
    }
}

// ============================================================================
// Self-wake detection tests (Decision 00-F1)
// ============================================================================
//
// A future that calls `cx.waker().wake_by_ref()` during poll and returns `Pending`
// signals "I progressed one step but need another turn" (turn-driven: stream-future
// pattern). `FutureTask` detects the self-wake token and returns `Pending(...)`
// (requeue) instead of `Depends` (park).
//
// A future that returns `Pending` without self-waking is event-driven (pipe-future
// pattern): it stashed the waker and waits for an external event. `FutureTask`
// returns `Depends` (park).

use foundation_core::valtron::FuturePollState;

/// WHY: A future that self-wakes (calls `wake_by_ref()` during poll) must produce
///      `Pending(FuturePollState::Pending)`, NOT `Depends`. The self-wake says
///      "re-poll me" — it is NOT waiting for an external event.
/// WHAT: Self-wake during first poll → Pending. Second poll → Ready(42).
#[test]
fn test_self_wake_returns_pending_not_depends() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct SelfWakeThenReady {
        polled: bool,
    }

    impl Future for SelfWakeThenReady {
        type Output = u32;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.polled {
                Poll::Ready(42)
            } else {
                self.polled = true;
                cx.waker().wake_by_ref(); // self-wake: "re-poll me"
                Poll::Pending
            }
        }
    }

    let mut task = FutureTask::new(SelfWakeThenReady { polled: false });

    // Poll 1: self-wake → Pending (requeue), NOT Depends.
    match task.next_status() {
        Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
        Some(TaskStatus::Depends(_)) => {
            panic!("self-wake must NOT return Depends — it is a requeue, not a park")
        }
        other => panic!("expected Pending(FuturePollState::Pending), got {other:?}"),
    }

    // Poll 2: Ready.
    match task.next_status() {
        Some(TaskStatus::Ready(42)) => {}
        other => panic!("expected Ready(42) on second poll, got {other:?}"),
    }

    assert!(task.next_status().is_none(), "task complete");
}

/// WHY: A future that returns `Pending` WITHOUT self-waking (stashes waker) must
///      still produce `Depends` with `is_ready = false`. The executor parks and
///      an external wake fires the readiness.
/// WHAT: Verify the event-driven path is unchanged.
#[test]
fn test_pending_without_self_wake_returns_depends_not_ready() {
    let future = core::future::pending::<i32>();
    let mut task = FutureTask::new(future);

    // Poll: no self-wake, no external wake → Depends(not-ready).
    match task.next_status() {
        Some(TaskStatus::Depends(signal)) => {
            assert!(
                !signal.is_ready(None),
                "empty queue, no self-wake → signal MUST be not-ready"
            );
        }
        other => panic!("expected Depends(not-ready), got {other:?}"),
    }
}

/// WHY: Stream-future bridges (like `StreamReadyFuture`) cycle through many
///      self-wake → Pending → requeue rounds before producing data. None of these
///      rounds should return `Depends(already_ready)` or trigger the executor's
///      `DEPENDS_TRUE_PANIC_THRESHOLD`.
/// WHAT: A future that self-wakes for N consecutive polls, then completes.
#[test]
fn test_self_wake_loop_no_depends_violations() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    const CYCLES: usize = 20;

    struct MultiSelfWake {
        remaining: usize,
    }

    impl Future for MultiSelfWake {
        type Output = usize;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.remaining == 0 {
                Poll::Ready(CYCLES)
            } else {
                self.remaining -= 1;
                cx.waker().wake_by_ref(); // advance one turn
                Poll::Pending
            }
        }
    }

    let mut task = FutureTask::new(MultiSelfWake { remaining: CYCLES });

    for i in 0..CYCLES {
        match task.next_status() {
            Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
            Some(TaskStatus::Depends(s)) => {
                panic!(
                    "cycle {i}/{CYCLES}: self-wake returned Depends (is_ready={}) — \
                     should be Pending. Each self-wake poll produces a token that \
                     FutureTask must detect and turn into Pending(None), not \
                     Depends(already_ready).",
                    s.is_ready(None)
                );
            }
            other => panic!("cycle {i}/{CYCLES}: expected Pending, got {other:?}"),
        }
    }

    // Final poll: Ready.
    match task.next_status() {
        Some(TaskStatus::Ready(CYCLES)) => {}
        other => panic!("expected Ready({CYCLES}) after {CYCLES} self-wake cycles, got {other:?}"),
    }

    assert!(task.next_status().is_none(), "task complete");
}

/// WHY: A future may alternate between self-wake (stream progress) and genuine
///      park (pipe data arrival). Both paths must coexist in one `FutureTask`.
/// WHAT: Two self-wake polls → Pending. Then stash waker → Depends(not-ready).
///       FutureTask handles the transition cleanly.
#[test]
fn test_mixed_self_wake_then_external_park() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct MixedFuture {
        stage: u8,
    }

    impl Future for MixedFuture {
        type Output = &'static str;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match self.stage {
                0 => {
                    self.stage = 1;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                1 => {
                    self.stage = 2;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                2 => {
                    self.stage = 3;
                    // Stash waker (via cx) — no self-wake this time.
                    let _ = cx.waker().clone();
                    Poll::Pending
                }
                _ => Poll::Ready("done"),
            }
        }
    }

    let mut task = FutureTask::new(MixedFuture { stage: 0 });

    assert!(matches!(task.next_status(), Some(TaskStatus::Pending(FuturePollState::Pending))));
    assert!(matches!(task.next_status(), Some(TaskStatus::Pending(FuturePollState::Pending))));

    // Third poll: no self-wake → Depends(not-ready).
    match task.next_status() {
        Some(TaskStatus::Depends(signal)) => {
            assert!(!signal.is_ready(None), "stashed waker, no external event → not ready");
        }
        other => panic!("expected Depends(not-ready), got {other:?}"),
    }
}

/// WHY: A future that stashes its waker (no self-wake) must return
///      `Depends(not-ready)`. When an external state change makes data available,
///      a subsequent poll (after the executor eventually re-runs the task) must
///      return `Ready`. This is the external-wake path — the pipe-future pattern.
///
///      `FutureTask.wake_queue` is private, so we cannot push a `WakeToken`
///      directly from the test to exercise the "token makes Depends ready" branch.
///      That branch is covered by `wake_before_park_is_never_lost` (self-wake path).
///      Here we verify that the queue-is-empty check correctly produces
///      `Depends(not-ready)` after a future stashes its waker, and that
///      re-polling after an external flag flip completes the future.
/// WHAT: Poll → Depends(not-ready). Flip flag. Poll → Ready.
#[test]
fn test_external_wake_makes_depends_ready() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct GateFuture {
        open: Arc<AtomicBool>,
    }

    impl Future for GateFuture {
        type Output = &'static str;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.open.load(Ordering::Acquire) {
                Poll::Ready("gated")
            } else {
                // Stash the waker (via clone) — no self-wake. An external entity
                // would fire this waker to signal data arrival.
                let _stashed = cx.waker().clone();
                Poll::Pending
            }
        }
    }

    let gate = Arc::new(AtomicBool::new(false));
    let mut task = FutureTask::new(GateFuture { open: gate.clone() });

    // Poll 1: gate closed → stash waker, no self-wake → Depends(not-ready).
    match task.next_status() {
        Some(TaskStatus::Depends(signal)) => {
            assert!(!signal.is_ready(None), "gate closed → not ready");
        }
        other => panic!("expected Depends(not-ready), got {other:?}"),
    }

    // External state change — gate opens.
    gate.store(true, Ordering::Release);

    // Poll 2: queue drained (still empty), future re-polled → gate open → Ready.
    match task.next_status() {
        Some(TaskStatus::Ready("gated")) => {}
        other => panic!("expected Ready(gated) after gate opened, got {other:?}"),
    }

    assert!(task.next_status().is_none(), "task complete");
}
