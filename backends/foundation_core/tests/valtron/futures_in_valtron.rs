/// WHY: FutureTask must poll and complete a simple future
/// WHAT: Future that returns Ready immediately should complete on first poll
#[test]
fn test_future_task_immediate_ready() {
    let future = core::future::ready(42);
    let mut task = FutureTask::new(future);

    // First poll should return Ready(42)
    match task.next_status() {
        Some(TaskStatus::Ready(42)) => {}
        other => panic!("Expected Ready(42), got {:?}", other),
    }

    // Task is complete
    assert!(task.next_status().is_none());
}

/// WHY: FutureTask must handle pending futures correctly
/// WHAT: Future that returns Pending should yield Pending status
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

    // First poll should be Pending
    match task.next_status() {
        Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
        other => panic!("Expected Pending, got {:?}", other),
    }

    // Second poll should be Ready
    match task.next_status() {
        Some(TaskStatus::Ready(100)) => {}
        other => panic!("Expected Ready(100), got {:?}", other),
    }

    // Task is complete
    assert!(task.next_status().is_none());
}

/// WHY: from_future() convenience function must work
/// WHAT: Creates FutureTask correctly
#[test]
fn test_from_future_convenience() {
    let future = core::future::ready("hello");
    let mut task = from_future(future);

    match task.next_status() {
        Some(TaskStatus::Ready("hello")) => {}
        other => panic!("Expected Ready(hello), got {:?}", other),
    }
}

/// WHY: No-op waker must be created successfully
/// WHAT: create_noop_waker() returns a valid Waker
#[test]
fn test_noop_waker_creation() {
    let waker = create_noop_waker();
    // Waker should be usable without panicking
    waker.wake_by_ref();
    let waker2 = waker.clone();
    waker2.wake();
}

/// WHY: get_noop_waker() must return a valid waker
/// WHAT: Function compiles and returns waker
#[test]
fn test_get_noop_waker() {
    let waker = get_noop_waker();
    waker.wake_by_ref();
}
