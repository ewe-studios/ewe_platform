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
