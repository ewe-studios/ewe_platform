//! Integration tests for valtron's cooperative sleep (`sleep`, `sleep_async`).

use std::time::{Duration, Instant};

use foundation_core::valtron::{Ctx, sleep_async, valtron_test, SleepingTask, TaskIterator};

// ── SleepingTask unit tests (no executor needed) ──────────────────────

#[test]
fn sleeping_task_elapsed_returns_none() {
    let ctx = Ctx::new();
    let mut t = SleepingTask::sleep(Duration::ZERO);
    assert!(t.next_status(ctx.clone()).is_none(), "already-elapsed sleep should be done");
}

#[test]
fn sleeping_task_future_yields_delayed() {
    let ctx = Ctx::new();
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut t = SleepingTask::new(deadline);
    let status = t.next_status(ctx.clone());
    assert!(
        matches!(status, Some(foundation_core::valtron::TaskStatus::Delayed(_))),
        "future deadline should yield Delayed"
    );
}

#[test]
fn sleeping_task_eventually_completes() {
    let ctx = Ctx::new();
    let mut t = SleepingTask::sleep(Duration::from_millis(1));
    // Drive until done — the Delayed loop should terminate
    for _ in 0..10 {
        match t.next_status(ctx.clone()) {
            None => return, // done
            Some(foundation_core::valtron::TaskStatus::Delayed(_)) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            _ => panic!("unexpected status"),
        }
    }
    panic!("SleepingTask did not complete within 10 iterations");
}

// ── sleep_async (Future via execute() + into_ready_future()) ──────────

#[valtron_test]
async fn sleep_async_completes() {
    let start = Instant::now();
    sleep_async(Duration::from_millis(50)).await;
    assert!(
        start.elapsed() >= Duration::from_millis(40),
        "sleep_async should wait at least ~40ms"
    );
}

#[valtron_test]
async fn sleep_async_zero_is_immediate() {
    let start = Instant::now();
    sleep_async(Duration::ZERO).await;
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "zero-duration sleep should return quickly"
    );
}

#[valtron_test]
async fn sleep_async_consecutive() {
    // Two consecutive sleeps should each wait
    let start = Instant::now();
    sleep_async(Duration::from_millis(30)).await;
    let first = start.elapsed();
    sleep_async(Duration::from_millis(30)).await;
    let total = start.elapsed();
    assert!(first >= Duration::from_millis(20), "first sleep too short");
    assert!(total >= Duration::from_millis(50), "consecutive sleeps too short");
}

// ── sleep_async needs no executor (native) ─────────────────────────────

/// A `Waker` that unparks the polling thread.
#[cfg(not(target_family = "wasm"))]
fn thread_waker() -> std::task::Waker {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    std::task::Waker::from(std::sync::Arc::new(Unpark(std::thread::current())))
}

/// `sleep_async` fires on its own, polled with no valtron pool at all. It used
/// to schedule a separate `sleep()` task (and `.expect` that it could), so as a
/// `select` timeout it depended on the pool running one more task — and hung
/// when the pool was busy or shutting down.
#[cfg(not(target_family = "wasm"))]
#[test]
fn sleep_async_fires_without_an_executor() {
    use std::future::Future;
    use std::task::{Context, Poll};

    let outcome = std::thread::spawn(|| {
        let waker = thread_waker();
        let mut cx = Context::from_waker(&waker);
        let started = std::time::Instant::now();
        let mut sleep = std::pin::pin!(sleep_async(Duration::from_millis(30)));
        loop {
            if let Poll::Ready(()) = sleep.as_mut().poll(&mut cx) {
                return started.elapsed();
            }
            assert!(started.elapsed() < Duration::from_secs(5), "never fired");
            std::thread::park_timeout(Duration::from_secs(1));
        }
    })
    .join();
    let elapsed = outcome.expect("sleep_async panicked without an executor");
    assert!(elapsed >= Duration::from_millis(30), "fired early: {elapsed:?}");
}

/// The timer wakes its waker at the deadline rather than needing a re-poll.
#[cfg(not(target_family = "wasm"))]
#[test]
fn timer_wakes_its_waker_at_the_deadline() {
    use std::future::Future;
    use std::task::{Context, Poll};

    let waker = thread_waker();
    let mut cx = Context::from_waker(&waker);
    let mut sleep = std::pin::pin!(foundation_core::valtron::timer::sleep_for(Duration::from_millis(20)));
    assert!(sleep.as_mut().poll(&mut cx).is_pending());
    let started = std::time::Instant::now();
    // One park: only the timer thread's wake (or the 5 s cap) ends it.
    std::thread::park_timeout(Duration::from_secs(5));
    assert!(started.elapsed() < Duration::from_secs(4), "the timer never woke us");
    assert_eq!(sleep.as_mut().poll(&mut cx), Poll::Ready(()));
}
