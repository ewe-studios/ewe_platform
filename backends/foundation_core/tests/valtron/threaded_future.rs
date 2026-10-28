//! Tests for `ThreadedIterFuture` executor.
//!
//! NOTE: these use `#[valtron_test]` alone (no `#[traced_test]`). `#[valtron_test]`
//! already installs a tracing subscriber, and `traced_test`'s
//! `set_global_default().expect(..)` panics with `SetGlobalDefaultError` once any
//! earlier test in the binary has set the global default. The tests here don't use
//! `logs_contain`/`logs_assert`, so `#[traced_test]` adds nothing.

#![cfg(feature = "multi")]

use foundation_core::valtron::{valtron_test, ThreadedIterFuture, ThreadedValue};

#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_basic() {

    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(())) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_future_error() {

    let threaded = ThreadedIterFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}

#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_empty_iterator() {

    let threaded = ThreadedIterFuture::new(|| async { Ok::<_, ()>(vec![].into_iter()) });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            ThreadedValue::Waiting => None,
        })
        .collect();

    assert!(results.is_empty());
}

#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_custom_queue_size() {

    // Use a queue size of 100 to hold all items, avoiding backpressure issues
    // in the test environment where thread scheduling may differ from production
    let threaded = ThreadedIterFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        100,
    );

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(())) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results.len(), 100);
}

/// Test that backpressure handling works correctly with a small queue size.
///
/// WHY: Verify the backpressure mechanism allows the producer to complete
/// even when the queue is much smaller than the total output
///
/// WHAT: Produces 100 items with a queue size of only 4, forcing many
/// backpressure cycles
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_backpressure() {

    // Small queue forces backpressure: 100 items with queue size 4
    // means ~96 backpressure cycles
    let threaded = ThreadedIterFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        4,
    );

    let iter = threaded.execute().expect("should submit job");

    // Collect results using the iterator directly, tolerating the `Waiting`
    // heartbeats the condvar path now yields between backpressure cycles.
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e:?}"),
            ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(
        results.len(),
        100,
        "Should have received all 100 items despite backpressure (got {})",
        results.len()
    );
}

/// A deterministic `Waiting` heartbeat: an inner iterator that sleeps ~3 s
/// before each value drains the queue empty long enough to exhaust the native
/// `max_spins = 100 × 10 ms` condvar budget, so the consumer provably yields
/// `ThreadedValue::Waiting` before the next value.
///
/// The gap is deliberately 3× the ~1 s condvar budget, not just above it. The
/// budget is measured in *spins*, and each spin's 10 ms `wait_timeout` can be
/// inflated by scheduler preemption when the full suite runs in parallel — so a
/// 1.2 s gap (200 ms of margin) flakes: the consumer's 100 spins take longer
/// than the gap and the value lands before the budget is exhausted.
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_yields_waiting_on_empty_queue() {
    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>((0..2).map(|i| {
            std::thread::sleep(std::time::Duration::from_millis(3000));
            Ok::<i32, ()>(i)
        }))
    });

    let iter = threaded.execute().expect("should submit job");

    let mut values = Vec::new();
    let mut saw_waiting = false;
    for item in iter {
        match item {
            ThreadedValue::Value(Ok(val)) => values.push(val),
            ThreadedValue::Value(Err(e)) => panic!("unexpected error: {e:?}"),
            ThreadedValue::Waiting => saw_waiting = true,
        }
    }

    assert_eq!(values, vec![0, 1]);
    assert!(
        saw_waiting,
        "a ~3 s empty-queue gap must yield at least one Waiting heartbeat"
    );
}

/// Pending until another thread wakes it, counting its polls.
struct WakeLater {
    polls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    started: bool,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl std::future::Future for WakeLater {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        use std::sync::atomic::Ordering;
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.done.load(Ordering::SeqCst) {
            return std::task::Poll::Ready(());
        }
        if !self.started {
            self.started = true;
            let waker = cx.waker().clone();
            let done = std::sync::Arc::clone(&self.done);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                done.store(true, Ordering::SeqCst);
                waker.wake();
            });
        }
        std::task::Poll::Pending
    }
}

/// The worker thread parks while its future is pending and resumes on the
/// future's wake. It used to poll with a no-op waker in a `yield_now` loop,
/// spinning a whole thread (thousands of polls in 100 ms).
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_parks_while_pending() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    let polls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&polls);
    let threaded = ThreadedIterFuture::new(move || async move {
        WakeLater {
            polls: counted,
            started: false,
            done: Arc::new(AtomicBool::new(false)),
        }
        .await;
        Ok::<_, ()>(vec![Ok::<i32, ()>(7)].into_iter())
    });

    let results: Vec<i32> = threaded
        .execute()
        .expect("should submit job")
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(())) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results, vec![7]);
    let polls = polls.load(Ordering::SeqCst);
    assert!(polls <= 5, "the pending future was polled {polls} times: busy-polling");
}
