// Integration tests for the `WakeableStream` wake side-channel (feature 02).
//
// These pin the producer-side firing contract and the forward/default combinator
// split. They are single-threaded and never block on a real consumer, so they are
// deterministic.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    NotifyQueue, NotifyQueueStreamIterator, Stream, StreamIteratorExt, WakeResult, WakeableStream,
};

/// A wake registered through a `NotifyQueueStreamIterator` must return `Successful`
/// and fire exactly once on the next `push`; a second registration fires exactly once
/// on the true open→closed transition, and a second `close` fires nothing further.
#[test]
fn notify_queue_wake_fires_on_push_and_close() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let mut iterator = NotifyQueueStreamIterator::new(queue.clone(), 5, Duration::from_micros(50));
    let counter = Arc::new(AtomicUsize::new(0));

    // Consumer registers a wake through the iterator.
    let c1 = Arc::clone(&counter);
    assert_eq!(
        iterator.wake(Box::new(move || {
            c1.fetch_add(1, Ordering::SeqCst);
        })),
        WakeResult::Successful
    );

    // `push` fires the registered callback exactly once.
    queue.push(Stream::Next(1usize)).unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Register a second callback for the close edge.
    let c2 = Arc::clone(&counter);
    assert_eq!(
        iterator.wake(Box::new(move || {
            c2.fetch_add(1, Ordering::SeqCst);
        })),
        WakeResult::Successful
    );

    // First `close` fires it (open → closed); a second `close` fires nothing further.
    assert!(queue.close());
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert!(!queue.close());
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

/// `FilterDone` (per-item) forwards `wake` to its inner waking terminal → `Successful`;
/// `into_stream_iter()` and `collect()` (buffering/fan-out) do not forward →
/// `NotSupported`.
#[test]
fn combinator_wake_forward_and_default_split() {
    // Forward: FilterDone reaches the waking terminal.
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyQueueStreamIterator::new(queue, 5, Duration::from_micros(50));
    let mut filtered = iterator.filter_done(|x| *x > 5);
    assert_eq!(filtered.wake(Box::new(|| {})), WakeResult::Successful);

    // Default: StreamIter (plain-iterator entry point) must NOT forward.
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyQueueStreamIterator::new(queue, 5, Duration::from_micros(50));
    let mut stream_iter = iterator.into_stream_iter();
    assert_eq!(stream_iter.wake(Box::new(|| {})), WakeResult::NotSupported);

    // Default: Collect (buffering) must NOT forward.
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyQueueStreamIterator::new(queue, 5, Duration::from_micros(50));
    let mut collected = StreamIteratorExt::collect_stream(iterator);
    assert_eq!(collected.wake(Box::new(|| {})), WakeResult::NotSupported);
}

/// `park_on` parks a task on a waking stream (`Depends` on a signal that turns
/// ready on the next push) and falls back to `Wait` for a stream that cannot
/// signal readiness.
#[test]
fn park_on_depends_on_a_waking_stream_and_waits_otherwise() {
    use foundation_core::valtron::{park_on, TaskStatus};

    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let mut iterator = NotifyQueueStreamIterator::new(queue.clone(), 5, Duration::from_micros(50));
    let status: TaskStatus<usize, ()> = park_on(&mut iterator);
    let TaskStatus::Depends(signal) = status else {
        panic!("a waking stream parks on Depends");
    };
    assert!(!signal.is_ready(None), "not ready before any data");
    queue.push(Stream::Next(1usize)).unwrap();
    assert!(signal.is_ready(None), "the push flips the signal");

    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let mut buffered =
        StreamIteratorExt::collect_stream(NotifyQueueStreamIterator::new(queue, 5, Duration::from_micros(50)));
    let status: TaskStatus<(), ()> = park_on(&mut buffered);
    assert!(matches!(status, TaskStatus::Wait), "no readiness signal: re-poll");
}

/// The first pull of an async bridge on an empty queue does not block: the
/// bridge announces itself (registers a wake) before pulling, so the queue
/// answers `Wait` and the future parks. It used to block the polling thread
/// for `max_spins` timed waits first — here 20 × 50 ms, a full second.
#[test]
fn an_async_bridge_does_not_block_on_its_first_empty_pull() {
    use foundation_core::valtron::TaskStatus;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyQueueStreamIterator::new(queue.clone(), 20, Duration::from_millis(50));
    let mut ready = std::pin::pin!(iterator.into_ready_future());
    let mut cx = Context::from_waker(Waker::noop());

    let started = std::time::Instant::now();
    assert!(ready.as_mut().poll(&mut cx).is_pending());
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "the first poll blocked for {:?}",
        started.elapsed()
    );

    queue.push(Stream::Next(3)).unwrap();
    match ready.as_mut().poll(&mut cx) {
        Poll::Ready(Some((value, _rest))) => assert_eq!(value, 3),
        other => panic!("expected the pushed value, got {:?}", other.map(|o| o.map(|(v, _)| v))),
    }

    // The task path: `TaskStatus::from_stream` parks at once too.
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let mut iterator = NotifyQueueStreamIterator::new(queue, 20, Duration::from_millis(50));
    let started = std::time::Instant::now();
    let status: Option<TaskStatus<usize, ()>> = TaskStatus::from_stream(&mut iterator);
    assert!(matches!(status, Some(TaskStatus::Depends(_))));
    assert!(started.elapsed() < Duration::from_millis(200), "from_stream blocked");
}
