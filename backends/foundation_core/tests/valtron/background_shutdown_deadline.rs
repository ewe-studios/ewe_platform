//! Regression tests for issue #233: `PoolGuard::shutdown_within` must bound the
//! background job registry, not only the task worker registry.
//!
//! WHY: `BackgroundJobRegistry::shutdown` applies its own configured timeout
//! (30 s by default). A background job stuck inside a closure used to hold the
//! process-wide pool lifecycle gate for that whole time, long past the caller's
//! `shutdown_within` argument.
//!
//! These tests use real threads and the monotonic clock. Each one gives itself a
//! hard upper bound with a channel receive timeout, so the old code fails at that
//! bound instead of hanging the suite for the configured 30 s.

#![cfg(feature = "multi")]

use foundation_core::valtron::multi::{initialize_pool_with_config, PoolConfig};
use foundation_core::valtron::{
    initialize_pool, run_background_job, BackgroundJobConfig, BackgroundJobRegistry,
    ShutdownOutcome,
};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Generous upper bound for a job or a pool to start. The tests wait on a
/// signal rather than sleeping a fixed time.
const START_DEADLINE: Duration = Duration::from_secs(10);

/// The test's own upper bound. On the old code `shutdown_within` waits for the
/// configured 30 s background timeout, so this fires first and fails the test.
const TEST_DEADLINE: Duration = Duration::from_secs(5);

/// Submit a background job that blocks until `release` is dropped, then wait
/// until it has started. Returns the sender that releases it.
fn start_stuck_background_job() -> mpsc::Sender<()> {
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();

    run_background_job(move || {
        let _ = started_tx.send(());
        let _ = release_rx.recv();
    })
    .expect("the background job should be accepted");

    started_rx
        .recv_timeout(START_DEADLINE)
        .expect("the blocking background job should start");
    release_tx
}

/// Run `shutdown_within(timeout)` on another thread and return how long it
/// took. Panics when the call does not return within [`TEST_DEADLINE`], so the
/// test fails at its own bound rather than hanging for the configured timeout.
fn time_shutdown_within(guard: foundation_core::valtron::PoolGuard, timeout: Duration) -> Duration {
    let (done_tx, done_rx) = mpsc::channel();
    let shutdown = thread::spawn(move || {
        let start = Instant::now();
        let _stopped = guard.shutdown_within(timeout);
        let took = start.elapsed();
        // Drop the guard before reporting, so the lifecycle gate is free even if
        // a later assertion fails.
        drop(guard);
        let _ = done_tx.send(took);
    });

    let took = done_rx.recv_timeout(TEST_DEADLINE).expect(
        "shutdown_within must return within the test deadline; the old code waits for the \
         configured 30 s background shutdown timeout",
    );
    shutdown.join().expect("shutdown thread");
    took
}

/// WHY: issue #233 — a stuck background job must not extend
/// `shutdown_within` past its argument.
/// WHAT: start a background job blocked on a channel, call
/// `shutdown_within(50 ms)`, and require the call to return in less than
/// 500 ms. On the old code it waits for the configured 30 s.
#[test]
fn shutdown_within_bounds_a_stuck_background_job() {
    let guard = initialize_pool(7, Some(3));
    let release = start_stuck_background_job();

    let took = time_shutdown_within(guard, Duration::from_millis(50));

    // Let the detached background worker finish now the measurement is done.
    drop(release);

    assert!(
        took < Duration::from_millis(500),
        "shutdown_within(50 ms) took {took:?}; a stuck background job outlived the bound"
    );
}

/// WHY: requirement 2 — the effective deadline is
/// `min(caller_timeout, config.background_shutdown_timeout)`. When the
/// configured value is smaller, it must win.
/// WHAT: configure the pool with a 100 ms background shutdown timeout, start a
/// stuck background job, and call `shutdown_within(10 s)`. The call must return
/// near 100 ms, not near 10 s.
#[test]
fn shutdown_within_uses_the_smaller_configured_background_timeout() {
    let guard = initialize_pool_with_config(
        8,
        Some(3),
        PoolConfig::default().with_background_shutdown_timeout(Duration::from_millis(100)),
    );
    let release = start_stuck_background_job();

    let took = time_shutdown_within(guard, Duration::from_secs(10));

    drop(release);

    assert!(
        took >= Duration::from_millis(100),
        "shutdown_within returned before the configured 100 ms background timeout: {took:?}"
    );
    assert!(
        took < Duration::from_secs(1),
        "shutdown_within(10 s) must use the smaller configured background timeout, took {took:?}"
    );
}

/// WHY: requirement 4 — the plain `shutdown()` keeps the configured timeout for
/// callers that do not bound it themselves.
/// WHAT: a registry with a 200 ms configured timeout and a stuck worker; plain
/// `shutdown()` waits at least the configured 200 ms and reports the stuck
/// worker.
#[test]
fn background_shutdown_alone_keeps_its_timeout() {
    let registry = BackgroundJobRegistry::with_config(
        BackgroundJobConfig::new(1).with_shutdown_timeout(Duration::from_millis(200)),
    )
    .expect("spawn workers");

    assert_eq!(
        registry.shutdown_timeout(),
        Duration::from_millis(200),
        "the registry must report the configured shutdown timeout"
    );

    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    registry
        .submit(move || {
            let _ = started_tx.send(());
            let _ = release_rx.recv();
        })
        .expect("submit the blocking job");
    started_rx
        .recv_timeout(START_DEADLINE)
        .expect("the blocking job should start");

    let start = Instant::now();
    let outcome = registry.shutdown(&foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let took = start.elapsed();
    drop(release_tx);

    assert_eq!(
        outcome,
        ShutdownOutcome::TimedOut {
            abandoned_jobs: 0,
            busy_workers: 1,
        },
        "a worker stuck inside a job must be reported as busy"
    );
    assert!(
        took >= Duration::from_millis(200),
        "plain shutdown() returned before the configured 200 ms timeout: {took:?}"
    );
    assert!(
        took < TEST_DEADLINE,
        "plain shutdown() must return at the configured timeout, took {took:?}"
    );
}
