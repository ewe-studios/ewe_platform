use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use foundation_core::valtron::{
    CancellableFutureTask, CancelOutcome, FuturePollState, Stream, TaskIterator, TaskStatus,
};

// ============================================================================
// Raw TaskIterator-level tests (no executor involved)
// ============================================================================

#[test]
fn immediate_ready_wraps_in_ok() {
    let cancel = Arc::new(AtomicBool::new(false));
    let future = core::future::ready(42);
    let mut task = CancellableFutureTask::new(future, cancel);

    match task.next_status() {
        Some(TaskStatus::Ready(Ok(42))) => {}
        other => panic!("expected Ready(Ok(42)), got {other:?}"),
    }

    assert!(task.next_status().is_none());
}

#[test]
fn pending_then_ready() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct PendingOnce(bool);

    impl Future for PendingOnce {
        type Output = &'static str;
        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.0 {
                Poll::Ready("done")
            } else {
                self.0 = true;
                Poll::Pending
            }
        }
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let mut task = CancellableFutureTask::new(PendingOnce(false), cancel);

    match task.next_status() {
        Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
        other => panic!("expected Pending, got {other:?}"),
    }

    match task.next_status() {
        Some(TaskStatus::Ready(Ok("done"))) => {}
        other => panic!("expected Ready(Ok(\"done\")), got {other:?}"),
    }

    assert!(task.next_status().is_none());
}

#[test]
fn cancel_before_first_poll() {
    let cancel = Arc::new(AtomicBool::new(true));
    let future = core::future::ready(99);
    let mut task = CancellableFutureTask::new(future, cancel);

    match task.next_status() {
        Some(TaskStatus::Ready(Err(CancelOutcome::Cancelled))) => {}
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

#[test]
fn cancel_after_pending() {
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct AlwaysPending;
    impl Future for AlwaysPending {
        type Output = i32;
        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let mut task = CancellableFutureTask::new(AlwaysPending, Arc::clone(&cancel));

    match task.next_status() {
        Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
        other => panic!("expected Pending, got {other:?}"),
    }

    cancel.store(true, Ordering::Release);

    match task.next_status() {
        Some(TaskStatus::Ready(Err(CancelOutcome::Cancelled))) => {}
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

#[test]
fn request_cancel_sets_flag() {
    let cancel = Arc::new(AtomicBool::new(false));
    let task = CancellableFutureTask::new(core::future::ready(()), Arc::clone(&cancel));

    assert!(!task.is_cancelled());
    task.request_cancel();
    assert!(task.is_cancelled());
    assert!(cancel.load(Ordering::Acquire));
}

#[test]
fn cancel_signal_returns_shared_arc() {
    let cancel = Arc::new(AtomicBool::new(false));
    let task = CancellableFutureTask::new(core::future::ready(()), Arc::clone(&cancel));

    let signal = task.cancel_signal();
    signal.store(true, Ordering::Release);
    assert!(task.is_cancelled());
    assert!(cancel.load(Ordering::Acquire));
}

// ============================================================================
// Executor-level tests — schedule via drive_cancellable_future
// ============================================================================

#[test]
fn drive_cancellable_future_completes() {
    let _guard = foundation_core::valtron::initialize_pool(42, None);

    let (mut stream, signal) =
        foundation_core::valtron::drive_cancellable_future(async { 7u32 }, None)
            .expect("schedule failed");

    assert!(!signal.load(Ordering::Acquire));

    let mut got_result = false;
    for item in &mut stream {
        if let Stream::Next(Ok(7)) = item {
            got_result = true;
            break;
        }
    }
    assert!(got_result);
}

#[test]
fn drive_cancellable_future_cancel_via_signal() {
    let _guard = foundation_core::valtron::initialize_pool(43, None);

    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct AlwaysPending;
    impl Future for AlwaysPending {
        type Output = i32;
        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    let (mut stream, signal) =
        foundation_core::valtron::drive_cancellable_future(AlwaysPending, None)
            .expect("schedule failed");

    // Let it get at least one Pending poll through the executor.
    let mut saw_pending = false;
    for item in &mut stream {
        match item {
            Stream::Pending(_) | Stream::Wait | Stream::Ignore | Stream::Init => {
                saw_pending = true;
                // Set cancel after seeing executor is running.
                signal.store(true, Ordering::Release);
                break;
            }
            Stream::Next(Err(CancelOutcome::Cancelled)) => {
                // Race: cancel was seen immediately.
                return;
            }
            _ => {}
        }
    }
    assert!(saw_pending);

    // Now drain until we see Cancelled.
    let mut cancelled = false;
    for item in &mut stream {
        if let Stream::Next(Err(CancelOutcome::Cancelled)) = item {
            cancelled = true;
            break;
        }
    }
    assert!(cancelled);
}

// ============================================================================
// Parallel execution tests
// ============================================================================

#[cfg(feature = "multi")]
#[test]
fn execute_cancellable_futures_parallel() {
    let _guard = foundation_core::valtron::initialize_pool(45, None);

    let futures = vec![
        core::future::ready(1u32),
        core::future::ready(2u32),
        core::future::ready(3u32),
    ];

    let (mut stream, signals) =
        foundation_core::valtron::execute_cancellable_futures(futures, None)
            .expect("schedule failed");

    assert_eq!(signals.len(), 3);

    let mut collected = Vec::new();
    for item in &mut stream {
        if let Stream::Next(Ok(v)) = item {
            collected.push(v);
            if collected.len() == 3 {
                break;
            }
        }
    }
    collected.sort();
    assert_eq!(collected, vec![1, 2, 3]);
}

#[cfg(feature = "multi")]
#[test]
fn execute_cancellable_futures_cancel_one() {
    let _guard = foundation_core::valtron::initialize_pool(46, None);

    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};

    struct AlwaysPending;
    impl Future for AlwaysPending {
        type Output = i32;
        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    let futures: Vec<core::pin::Pin<Box<dyn Future<Output = i32> + Send>>> = vec![
        Box::pin(core::future::ready(10)),
        Box::pin(AlwaysPending),
    ];

    let (mut stream, signals) =
        foundation_core::valtron::execute_cancellable_futures(futures, None)
            .expect("schedule failed");

    assert_eq!(signals.len(), 2);

    // Cancel the always-pending future.
    signals[1].store(true, Ordering::Release);

    let mut results = Vec::new();
    for item in &mut stream {
        match item {
            Stream::Next(Ok(v)) => results.push(v),
            Stream::Next(Err(CancelOutcome::Cancelled)) => results.push(-1),
            _ => {}
        }
        if results.len() == 2 {
            break;
        }
    }
    results.sort();
    assert_eq!(results, vec![-1, 10]);
}
