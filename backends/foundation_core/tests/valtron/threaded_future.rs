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

// ---------------------------------------------------------------------------
// Parking: CPU time of the worker thread
// ---------------------------------------------------------------------------

/// CPU time used so far by the calling thread, in nanoseconds.
#[cfg(unix)]
fn thread_cpu_nanos() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out pointer; CLOCK_THREAD_CPUTIME_ID is supported
    // on every unix target this crate tests on.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Pending for 300 ms until another thread wakes it. Records the CPU time of
/// the thread that polls it at its first and last poll.
#[cfg(unix)]
struct SlowWake {
    started: bool,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    cpu_first: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cpu_last: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[cfg(unix)]
impl std::future::Future for SlowWake {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        use std::sync::atomic::Ordering;
        let now = thread_cpu_nanos();
        if !self.started {
            self.started = true;
            self.cpu_first.store(now, Ordering::SeqCst);
            let waker = cx.waker().clone();
            let done = std::sync::Arc::clone(&self.done);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                done.store(true, Ordering::SeqCst);
                waker.wake();
            });
        }
        self.cpu_last.store(now, Ordering::SeqCst);
        if self.done.load(Ordering::SeqCst) {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}

/// A future that is pending for 300 ms must cost the worker thread (almost) no
/// CPU time: it parks and the waker unparks it. The old code polled with a
/// no-op waker in a `yield_now` loop and used close to 300 ms of CPU.
#[cfg(unix)]
#[valtron_test(seed = 42, threads = 4, timeout = 20_000)]
fn threaded_iter_future_does_not_spin_while_pending() {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;

    let cpu_first = Arc::new(AtomicU64::new(0));
    let cpu_last = Arc::new(AtomicU64::new(0));
    let (first, last) = (Arc::clone(&cpu_first), Arc::clone(&cpu_last));

    let threaded = ThreadedIterFuture::new(move || async move {
        SlowWake {
            started: false,
            done: Arc::new(AtomicBool::new(false)),
            cpu_first: first,
            cpu_last: last,
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

    let used_ms = (cpu_last.load(Ordering::SeqCst) - cpu_first.load(Ordering::SeqCst)) / 1_000_000;
    assert!(used_ms < 50, "the worker thread used {used_ms} ms of CPU while its future was pending");
}

/// Yields 5 items, then records the CPU time of the polling thread when it is
/// asked for a sixth (that is after the last push finished waiting).
#[cfg(unix)]
struct FiveItems {
    next: i32,
    cpu_start: u64,
    cpu_end: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cpu_begin: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[cfg(unix)]
impl Iterator for FiveItems {
    type Item = Result<i32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::sync::atomic::Ordering;
        if self.next == 0 {
            self.cpu_start = thread_cpu_nanos();
            self.cpu_begin.store(self.cpu_start, Ordering::SeqCst);
        }
        if self.next == 5 {
            self.cpu_end.store(thread_cpu_nanos(), Ordering::SeqCst);
            return None;
        }
        self.next += 1;
        Some(Ok(self.next))
    }
}

/// A producer that meets a full queue parks until the consumer takes an item.
/// Queue of 2 (the smallest `ThreadedIterFuture` allows), no back-pressure
/// timeout, a consumer that takes one item every 100 ms: the order and count
/// are kept and the producer thread uses almost no CPU while it waits. The old
/// code spun in `spin_loop` when no sleep was configured.
#[cfg(unix)]
#[valtron_test(seed = 42, threads = 4, timeout = 20_000)]
fn threaded_iter_future_backpressure_parks_until_space() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let cpu_begin = Arc::new(AtomicU64::new(0));
    let cpu_end = Arc::new(AtomicU64::new(0));
    let (begin, end) = (Arc::clone(&cpu_begin), Arc::clone(&cpu_end));

    let threaded = ThreadedIterFuture::with_backpressure_sleep(
        move || async move {
            Ok::<_, ()>(FiveItems { next: 0, cpu_start: 0, cpu_end: end, cpu_begin: begin })
        },
        2,
        None,
    );

    let mut got = Vec::new();
    for value in threaded.execute().expect("should submit job") {
        if let ThreadedValue::Value(Ok(v)) = value {
            got.push(v);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    assert_eq!(got, vec![1, 2, 3, 4, 5]);

    let used_ms = (cpu_end.load(Ordering::SeqCst) - cpu_begin.load(Ordering::SeqCst)) / 1_000_000;
    assert!(used_ms < 50, "the producer used {used_ms} ms of CPU while the queue was full");
}

/// An `Err` from the future reaches the consumer even when the consumer is slow
/// to start pulling (it only begins after 300 ms). Note: the queue is always
/// empty when the future's error is pushed (nothing is pushed before it and the
/// smallest queue holds 2), so the old code could not drop the error either;
/// this test pins the delivery contract of `push_waiting`'s error path.
#[valtron_test(seed = 42, threads = 4, timeout = 20_000)]
fn threaded_iter_future_error_reaches_consumer_when_queue_full() {
    let threaded = ThreadedIterFuture::with_backpressure_sleep(
        || async { Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed") },
        2,
        None,
    );
    let iter = threaded.execute().expect("should submit job");
    std::thread::sleep(std::time::Duration::from_millis(300));

    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            ThreadedValue::Waiting => None,
        })
        .collect();
    assert_eq!(results, vec![Err("future failed")]);
}
