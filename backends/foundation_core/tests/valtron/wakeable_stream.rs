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

/// Counts how often it is woken.
struct CountWake(AtomicUsize);

impl std::task::Wake for CountWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// An async consumer's first poll on an empty queue returns `Pending` at once
/// (the old code blocked the polling thread for `max_spins` timed waits: here
/// 20 x 200 ms), registers a real wake, and a later push wakes it and delivers
/// the item.
#[test]
fn first_empty_pull_does_not_block() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyQueueStreamIterator::new(queue.clone(), 20, Duration::from_millis(200));
    let mut ready = std::pin::pin!(iterator.into_ready_future());

    let counter = Arc::new(CountWake(AtomicUsize::new(0)));
    let waker = Waker::from(counter.clone());
    let mut cx = Context::from_waker(&waker);

    let started = std::time::Instant::now();
    assert!(ready.as_mut().poll(&mut cx).is_pending());
    assert!(
        started.elapsed() < Duration::from_millis(5),
        "the first empty pull blocked for {:?}",
        started.elapsed()
    );
    assert_eq!(counter.0.load(Ordering::SeqCst), 0);

    queue.push(Stream::Next(9)).unwrap();
    assert_eq!(counter.0.load(Ordering::SeqCst), 1, "the push must wake the consumer");
    match ready.as_mut().poll(&mut cx) {
        Poll::Ready(Some((value, _rest))) => assert_eq!(value, 9),
        _ => panic!("expected the pushed value"),
    }
}

/// A stream whose producer pushes an item exactly between the consumer's empty
/// pull and the registration of its real wake (the push fires only the wakes
/// registered so far, so it never fires the real one).
struct RacingStream {
    inner: NotifyQueueStreamIterator<usize, ()>,
    queue: Arc<NotifyQueue<Stream<usize, ()>>>,
    saw_empty: bool,
    pushed: bool,
}

impl Iterator for RacingStream {
    type Item = Stream<usize, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next();
        if matches!(item, Some(Stream::Wait)) {
            self.saw_empty = true;
        }
        item
    }
}

impl WakeableStream for RacingStream {
    fn wake(&mut self, cb: foundation_core::valtron::WakeCallback) -> WakeResult {
        // The first registration after an empty pull is the real one (a
        // priming call comes before any pull and does not count).
        if self.saw_empty && !self.pushed {
            self.pushed = true;
            self.queue.push(Stream::Next(5)).unwrap();
        }
        self.inner.wake(cb)
    }

    fn announce(&mut self) {
        // Forward the async-consumer announcement so the inner first empty
        // pull answers `Wait` (setting `saw_empty`) instead of blocking.
        self.inner.announce();
    }
}

/// An item pushed after the empty pull and before the real wake is registered
/// is not lost: the consumer pulls once more after registering and returns it
/// in the same poll.
#[test]
fn item_pushed_between_empty_pull_and_registration_is_not_lost() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let inner = NotifyQueueStreamIterator::new(queue.clone(), 20, Duration::from_millis(200));
    let racing = RacingStream { inner, queue, saw_empty: false, pushed: false };
    let mut ready = std::pin::pin!(racing.into_ready_future());
    let mut cx = Context::from_waker(Waker::noop());

    match ready.as_mut().poll(&mut cx) {
        Poll::Ready(Some((value, _rest))) => assert_eq!(value, 5),
        Poll::Ready(None) => panic!("stream ended"),
        Poll::Pending => panic!("the item pushed before the registration was lost"),
    }
}

/// A stream that always answers `Ignore` and forwards `wake` to a queue. It
/// makes every no-op wake registered per `TaskStatus::from_stream` poll visible
/// through [`NotifyQueue::pending_wakes`].
struct IgnoreStream {
    queue: Arc<NotifyQueue<Stream<usize, ()>>>,
}

impl Iterator for IgnoreStream {
    type Item = Stream<usize, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Stream::Ignore)
    }
}

impl WakeableStream for IgnoreStream {
    fn wake(&mut self, cb: foundation_core::valtron::WakeCallback) -> WakeResult {
        self.queue.register_wake(cb);
        WakeResult::Successful
    }
}

/// `TaskStatus::from_stream` announces (not wakes) before the pull, so repeated
/// polls of a stream that never parks must not pile up no-op wake callbacks.
#[test]
fn from_stream_does_not_accumulate_noop_wakes() {
    use foundation_core::valtron::TaskStatus;

    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let mut stream = IgnoreStream { queue: queue.clone() };

    for _ in 0..100 {
        let status: Option<TaskStatus<usize, ()>> = TaskStatus::from_stream(&mut stream);
        assert!(matches!(status, Some(TaskStatus::Ignore)));
    }

    assert_eq!(
        queue.pending_wakes(),
        0,
        "from_stream must not register a wake callback per poll"
    );
}

/// A producer re-registering its space wake on each retry of a full push must
/// replace the earlier callback, not pile up stale ones.
#[test]
fn register_space_wake_replaces_not_accumulates() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::bounded(1));
    queue.push(Stream::Next(1usize)).unwrap();
    assert!(queue.queue().is_full(), "a capacity-1 queue with one item is full");

    for _ in 0..100 {
        queue.register_space_wake(Box::new(|| {}));
    }

    assert_eq!(
        queue.pending_space_wakes(),
        1,
        "a re-registered space wake must replace the earlier one"
    );
}

/// Taking an item from a full queue must wake a producer that registered a space
/// wake (no lost wake through the `has_waiters` fast path).
#[test]
fn pop_wakes_a_waiting_producer() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::bounded(1));
    queue.push(Stream::Next(1usize)).unwrap();

    let fired = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&fired);
    queue.register_space_wake(Box::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
    }));

    assert_eq!(fired.load(Ordering::SeqCst), 0, "not woken before a pop frees space");
    assert!(matches!(queue.pop(), Ok(Stream::Next(1))), "pop returns the pushed item");
    assert_eq!(fired.load(Ordering::SeqCst), 1, "pop must wake a registered space waiter");
}
