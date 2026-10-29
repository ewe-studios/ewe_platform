//! Tests for `BackgroundJobRegistry` and `split_thread_count`.

use foundation_core::valtron::{
    split_thread_count, BackgroundJobConfig, BackgroundJobRegistry, ShutdownOutcome,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

/// Generous upper bound for a background job to run. The tests wait on a
/// signal rather than sleeping a fixed time, so they stay fast when idle and
/// don't flake when the rest of the test binary is loading the machine.
const JOB_DEADLINE: Duration = Duration::from_secs(10);

/// WHY: `split_thread_count` must follow the specified allocation table
/// WHAT: Verify `(task_threads, bg_threads)` for known input values
#[test]
fn test_split_thread_count() {
    // bg = total, task = max(2, total / 2)
    assert_eq!(split_thread_count(3), (2, 3));
    assert_eq!(split_thread_count(4), (2, 4));
    assert_eq!(split_thread_count(5), (2, 5));
    assert_eq!(split_thread_count(6), (3, 6));
    assert_eq!(split_thread_count(7), (3, 7));
    assert_eq!(split_thread_count(8), (4, 8));
    assert_eq!(split_thread_count(10), (5, 10));
    assert_eq!(split_thread_count(12), (6, 12));
    assert_eq!(split_thread_count(16), (8, 16));
}

#[test]
#[should_panic(expected = "split_thread_count requires at least 3 threads")]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding"
)]
fn test_split_thread_count_panics_below_3() {
    let _ = split_thread_count(1);
    let _ = split_thread_count(2);
}

/// WHY: `BackgroundJobRegistry` must execute submitted jobs
/// WHAT: Submit a job that signals a channel, verify it ran
#[test]
fn test_submit_and_execute_job() {
    let registry = BackgroundJobRegistry::new(2).expect("spawn workers");

    let (done_tx, done_rx) = mpsc::channel();
    registry
        .submit(move || {
            done_tx.send(()).expect("test receiver alive");
        })
        .expect("submit should succeed");

    assert!(
        done_rx.recv_timeout(JOB_DEADLINE).is_ok(),
        "job should have executed"
    );

    assert_eq!(registry.shutdown(), ShutdownOutcome::Completed);
}

/// WHY: Workers must survive panicking jobs
/// WHAT: Submit a panicking job, then a normal job — both should be handled
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding"
)]
fn test_panic_recovery() {
    // One worker: the job after the panic can only run if that same worker
    // survived the panic.
    let registry = BackgroundJobRegistry::new(1).expect("spawn workers");

    registry
        .submit(|| {
            panic!("intentional test panic");
        })
        .expect("submit should succeed");

    let (done_tx, done_rx) = mpsc::channel();
    registry
        .submit(move || {
            done_tx.send(()).expect("test receiver alive");
        })
        .expect("submit should succeed after panic");

    assert!(
        done_rx.recv_timeout(JOB_DEADLINE).is_ok(),
        "worker should recover from panic and execute next job"
    );

    assert_eq!(registry.shutdown(), ShutdownOutcome::Completed);
}

/// WHY: shutdown must run every accepted job, then let workers exit cleanly
/// WHAT: Submit 10 jobs and shut down straight away; all 10 have run once
/// shutdown returns
#[test]
fn test_shutdown() {
    let registry = BackgroundJobRegistry::new(2).expect("spawn workers");

    let counter = Arc::new(AtomicUsize::new(0));
    for _ in 0..10 {
        let c = counter.clone();
        registry
            .submit(move || {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .expect("submit should succeed");
    }

    assert_eq!(registry.shutdown(), ShutdownOutcome::Completed);

    assert_eq!(
        counter.load(Ordering::SeqCst),
        10,
        "all 10 jobs should have executed"
    );
}

/// WHY: submit must fail after shutdown
/// WHAT: Shut down registry, try to submit — should return Err
#[test]
fn test_submit_after_shutdown_fails() {
    let registry = BackgroundJobRegistry::new(1).expect("spawn workers");

    assert_eq!(registry.shutdown(), ShutdownOutcome::Completed);

    let result = registry.submit(|| {});
    assert!(result.is_err(), "submit after shutdown should fail");
}

/// A registry with one worker that is busy in a job which blocks until the
/// returned sender is used (or dropped). `on_release` runs inside that job
/// right after it is released, still on the worker.
fn registry_with_blocked_worker(
    on_release: impl FnOnce() + Send + 'static,
) -> (Arc<BackgroundJobRegistry>, mpsc::Sender<()>) {
    registry_with_blocked_worker_config(BackgroundJobConfig::new(1), on_release)
}

/// [`registry_with_blocked_worker`] with an explicit config.
fn registry_with_blocked_worker_config(
    config: BackgroundJobConfig,
    on_release: impl FnOnce() + Send + 'static,
) -> (Arc<BackgroundJobRegistry>, mpsc::Sender<()>) {
    let registry = BackgroundJobRegistry::with_config(config).expect("spawn workers");
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    registry
        .submit(move || {
            started_tx.send(()).expect("test alive");
            // Err means the test dropped the sender: release either way.
            let _released = release_rx.recv();
            on_release();
        })
        .expect("submit blocker");
    started_rx
        .recv_timeout(JOB_DEADLINE)
        .expect("blocker job should start");
    (registry, release_tx)
}

/// Start `shutdown()` on another thread and return once it has closed the
/// queue (observed by `submit` being rejected).
fn begin_shutdown(registry: &Arc<BackgroundJobRegistry>) -> thread::JoinHandle<ShutdownOutcome> {
    let shutdown = {
        let registry = registry.clone();
        thread::spawn(move || registry.shutdown())
    };
    let deadline = Instant::now() + JOB_DEADLINE;
    while registry.submit(|| {}).is_ok() {
        assert!(Instant::now() < deadline, "shutdown never closed the queue");
        thread::yield_now();
    }
    shutdown
}

/// WHY: shutdown used to drop jobs that were still queued: workers exited as
/// soon as the kill signal was set, so an accepted job could silently never
/// run. Accepted jobs must run even when shutdown starts before they do.
/// WHAT: The only worker is busy; 5 jobs wait behind it; shutdown starts and
/// closes the queue; then the worker is released. All 5 must have run by the
/// time shutdown returns.
#[test]
fn test_shutdown_runs_jobs_still_queued() {
    let (registry, release) = registry_with_blocked_worker(|| {});

    let ran = Arc::new(AtomicUsize::new(0));
    for _ in 0..5 {
        let ran = ran.clone();
        registry
            .submit(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .expect("submit before shutdown");
    }

    let shutdown = begin_shutdown(&registry);
    release.send(()).expect("blocker alive");
    assert_eq!(
        shutdown.join().expect("shutdown thread"),
        ShutdownOutcome::Completed
    );

    assert_eq!(
        ran.load(Ordering::SeqCst),
        5,
        "every job accepted before shutdown must run"
    );
}

/// WHY: shutdown closes intake for everyone, including jobs that are still
/// draining — the documented semantics
/// WHAT: A job running while shutdown is in progress tries to submit a
/// follow-up; it is rejected and the follow-up never runs.
#[test]
fn test_submit_from_draining_job_is_rejected() {
    let (result_tx, result_rx) = mpsc::channel();
    let follow_up_ran = Arc::new(AtomicUsize::new(0));
    let registry_slot: Arc<std::sync::OnceLock<Arc<BackgroundJobRegistry>>> =
        Arc::new(std::sync::OnceLock::new());

    let (registry, release) = {
        let slot = registry_slot.clone();
        let follow_up_ran = follow_up_ran.clone();
        registry_with_blocked_worker(move || {
            let registry = slot.get().expect("registry set before release");
            let result = registry.submit(move || {
                follow_up_ran.fetch_add(1, Ordering::SeqCst);
            });
            result_tx.send(result.is_err()).expect("test alive");
        })
    };
    assert!(registry_slot.set(registry.clone()).is_ok());

    let shutdown = begin_shutdown(&registry);
    release.send(()).expect("blocker alive");
    assert_eq!(
        shutdown.join().expect("shutdown thread"),
        ShutdownOutcome::Completed
    );

    assert!(
        result_rx.recv_timeout(JOB_DEADLINE).expect("job reported"),
        "submit from a draining job must be rejected"
    );
    assert_eq!(follow_up_ran.load(Ordering::SeqCst), 0);
}

/// WHY: idle workers used to sleep-poll (`thread::sleep` of 50 ms in the
/// pool), so every job submitted to an idle pool waited for the next poll.
/// Workers must instead block and be woken by `submit`.
/// WHAT: 20 sequential submit-and-wait round trips to a single idle worker.
/// With a 50 ms poll that takes ~1 s; woken workers take a few ms. 500 ms
/// leaves a wide margin for a loaded machine.
#[test]
fn test_idle_worker_is_woken_by_submit() {
    let registry = BackgroundJobRegistry::new(1).expect("spawn workers");

    let start = Instant::now();
    for _ in 0..20 {
        let (done_tx, done_rx) = mpsc::channel();
        registry
            .submit(move || done_tx.send(()).expect("test alive"))
            .expect("submit");
        done_rx.recv_timeout(JOB_DEADLINE).expect("job should run");
    }
    let elapsed = start.elapsed();
    assert_eq!(registry.shutdown(), ShutdownOutcome::Completed);

    assert!(
        elapsed < Duration::from_millis(500),
        "20 round trips to an idle worker took {elapsed:?}"
    );
}

/// WHY: shutting down an idle pool must not wait out a poll interval either
/// WHAT: Shutdown of a pool whose workers are all idle returns promptly
#[test]
fn test_idle_shutdown_is_prompt() {
    let registry = BackgroundJobRegistry::new(4).expect("spawn workers");
    let start = Instant::now();
    assert_eq!(registry.shutdown(), ShutdownOutcome::Completed);
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "idle shutdown took {elapsed:?}"
    );
}

/// WHY: shutdown waits for accepted jobs, so a job that never returns used to
/// block it forever. It must give up at the configured deadline instead.
/// WHAT: The only worker is stuck in a job; 3 jobs are queued behind it; the
/// shutdown timeout is 200 ms. Shutdown must return well before the stuck job
/// ends, report the stuck worker and the 3 dropped jobs, and never run them.
#[test]
fn test_shutdown_gives_up_at_the_timeout() {
    let (registry, release) = registry_with_blocked_worker_config(
        BackgroundJobConfig::new(1).with_shutdown_timeout(Duration::from_millis(200)),
        || {},
    );

    let ran = Arc::new(AtomicUsize::new(0));
    for _ in 0..3 {
        let ran = ran.clone();
        registry
            .submit(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .expect("submit before shutdown");
    }

    let (outcome_tx, outcome_rx) = mpsc::channel();
    let shutdown_registry = registry.clone();
    thread::spawn(move || {
        let start = Instant::now();
        let outcome = shutdown_registry.shutdown();
        outcome_tx
            .send((outcome, start.elapsed()))
            .expect("test alive");
    });
    let (outcome, took) = outcome_rx
        .recv_timeout(JOB_DEADLINE)
        .expect("shutdown must not block on a job that never returns");

    assert_eq!(
        outcome,
        ShutdownOutcome::TimedOut {
            abandoned_jobs: 3,
            busy_workers: 1,
        }
    );
    assert!(
        took >= Duration::from_millis(200),
        "returned before the timeout: {took:?}"
    );

    // The stuck job ends after shutdown gave up: the dropped jobs stay dropped.
    release.send(()).expect("blocker alive");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(ran.load(Ordering::SeqCst), 0, "dropped jobs must not run");
}

/// WHY: the timeout bounds shutdown; it must not cut short jobs that finish
/// within it
/// WHAT: A worker busy for ~50 ms with 2 jobs queued, 5 s timeout: shutdown
/// completes and every job runs.
#[test]
fn test_shutdown_within_timeout_completes() {
    let (registry, release) = registry_with_blocked_worker_config(
        BackgroundJobConfig::new(1).with_shutdown_timeout(Duration::from_secs(5)),
        || thread::sleep(Duration::from_millis(50)),
    );
    let ran = Arc::new(AtomicUsize::new(0));
    for _ in 0..2 {
        let ran = ran.clone();
        registry
            .submit(move || {
                ran.fetch_add(1, Ordering::SeqCst);
            })
            .expect("submit before shutdown");
    }

    let shutdown = begin_shutdown(&registry);
    release.send(()).expect("blocker alive");
    assert_eq!(
        shutdown.join().expect("shutdown thread"),
        ShutdownOutcome::Completed
    );
    assert_eq!(ran.load(Ordering::SeqCst), 2);
}

/// WHY: the default deadline is documented; keep the code and docs in step
#[test]
fn test_default_shutdown_timeout() {
    let config = BackgroundJobConfig::new(2);
    assert_eq!(config.num_threads(), 2);
    assert_eq!(config.shutdown_timeout(), Duration::from_secs(30));
    assert_eq!(
        config.shutdown_timeout(),
        BackgroundJobConfig::DEFAULT_SHUTDOWN_TIMEOUT
    );
}
