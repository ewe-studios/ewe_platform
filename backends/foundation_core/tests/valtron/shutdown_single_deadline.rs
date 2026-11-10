//! Regression test for issue #340: `PoolGuard::shutdown_within` must bound
//! both shutdown phases by one deadline.
//!
//! WHY: the background job registry phase and the task worker registry phase
//! each received the caller's full `timeout`. With a stuck job and a busy
//! worker in the same call, one `shutdown_within(timeout)` could take about
//! `2 * timeout` plus scheduler delay.
//!
//! The test makes both phases busy, then measures one call. The old code takes
//! about two seconds for a one-second budget; the fixed code takes about one
//! second. A channel receive timeout gives the call its own hard upper bound,
//! so a regression cannot hang the suite.

#![cfg(feature = "multi")]

use foundation_context::Ctx;
use foundation_core::valtron::multi::{initialize_pool_with_config, PoolConfig};
use foundation_core::valtron::{run_background_job, send, PoolGuard, TaskIterator, TaskStatus};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Generous upper bound for a job or a worker to start. The test waits on a
/// signal rather than sleeping a fixed time.
const START_DEADLINE: Duration = Duration::from_secs(10);

/// The test's own upper bound on the shutdown call. The old code finishes in
/// about two seconds, so this bound is a hang guard, not the assertion.
const TEST_DEADLINE: Duration = Duration::from_secs(5);

/// A task whose single step blocks its worker until released, like a task
/// stuck in a blocking syscall or a deadlock.
struct BlockingTask {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl TaskIterator for BlockingTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let _ = self.started.send(());
        // Err (sender gone) also ends the wait; either way the task is done.
        let _ = self.release.recv();
        None
    }
}

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

/// Schedule a task that blocks one task worker, then wait until it has
/// started. Returns the sender that releases it.
fn start_busy_task_worker() -> mpsc::Sender<()> {
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();

    send(BlockingTask {
        started: started_tx,
        release: release_rx,
    })
    .expect("the blocking task should be scheduled");

    started_rx
        .recv_timeout(START_DEADLINE)
        .expect("the blocking task should start on a worker");
    release_tx
}

/// Run `shutdown_within(timeout)` on another thread and return how long it
/// took. Panics when the call does not return within [`TEST_DEADLINE`], so the
/// test fails at its own bound rather than hanging.
fn time_shutdown_within(guard: PoolGuard, timeout: Duration) -> Duration {
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
        "shutdown_within must return within the test deadline; a regression must \
         not hang the suite",
    );
    shutdown.join().expect("shutdown thread");
    took
}

/// WHY: issue #340 — one call must share one deadline across both shutdown
/// phases. A stuck background job and a busy task worker in the same call must
/// not each consume the full budget.
/// WHAT: with a 5 s configured background timeout, busy background and task
/// phases, call `shutdown_within(1 s)`. The call must return in about one
/// second. The old code takes about two seconds.
#[test]
fn shutdown_within_bounds_both_phases_by_one_deadline() {
    let guard = initialize_pool_with_config(
        21,
        Some(1),
        PoolConfig::default().with_background_shutdown_timeout(Duration::from_secs(5)),
    );
    // Start both phases before the measurement: one background job stuck in a
    // closure, and one task worker stuck inside a task.
    let release_background = start_stuck_background_job();
    let release_worker = start_busy_task_worker();

    let took = time_shutdown_within(guard, Duration::from_secs(1));

    // Let the detached background worker and task worker finish now the
    // measurement is done.
    drop(release_background);
    drop(release_worker);

    assert!(
        took < Duration::from_millis(1600),
        "shutdown_within(1 s) took {took:?}; the two phases each spent the full budget"
    );
    assert!(
        took >= Duration::from_millis(900),
        "shutdown_within returned before its one-second budget while both phases \
         were busy: {took:?}"
    );
}
