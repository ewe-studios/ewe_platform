//! Tests for `BackgroundJobRegistry` and `split_thread_count`.

use foundation_core::synca::OnSignal;
use foundation_core::valtron::{split_thread_count, BackgroundJobRegistry};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Generous upper bound for a background job to run. The tests wait on a
/// signal rather than sleeping a fixed time, so they stay fast when idle and
/// don't flake when the rest of the test binary is loading the machine.
const JOB_DEADLINE: Duration = Duration::from_secs(10);

/// WHY: split_thread_count must follow the specified allocation table
/// WHAT: Verify (task_threads, bg_threads) for known input values
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

/// WHY: BackgroundJobRegistry must execute submitted jobs
/// WHAT: Submit a job that signals a channel, verify it ran
#[test]
fn test_submit_and_execute_job() {
    let kill_signal = Arc::new(OnSignal::new());
    let registry = BackgroundJobRegistry::new(2, kill_signal.clone(), Duration::from_millis(10));

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

    registry.shutdown();
}

/// WHY: Workers must survive panicking jobs
/// WHAT: Submit a panicking job, then a normal job — both should be handled
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding"
)]
fn test_panic_recovery() {
    let kill_signal = Arc::new(OnSignal::new());
    // One worker: the job after the panic can only run if that same worker
    // survived the panic.
    let registry = BackgroundJobRegistry::new(1, kill_signal.clone(), Duration::from_millis(10));

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

    registry.shutdown();
}

/// WHY: shutdown must cause workers to exit and join cleanly
/// WHAT: Create registry, run work, shutdown, verify no hanging threads
#[test]
fn test_shutdown() {
    let kill_signal = Arc::new(OnSignal::new());
    let registry = BackgroundJobRegistry::new(2, kill_signal.clone(), Duration::from_millis(10));

    let counter = Arc::new(AtomicUsize::new(0));
    let (done_tx, done_rx) = mpsc::channel();
    for _ in 0..10 {
        let c = counter.clone();
        let done_tx = done_tx.clone();
        registry
            .submit(move || {
                c.fetch_add(1, Ordering::SeqCst);
                done_tx.send(()).expect("test receiver alive");
            })
            .expect("submit should succeed");
    }

    // shutdown() closes the queue without draining it, so wait for every job
    // to have run before shutting down.
    for _ in 0..10 {
        assert!(
            done_rx.recv_timeout(JOB_DEADLINE).is_ok(),
            "all 10 jobs should have executed"
        );
    }

    registry.shutdown();

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
    let kill_signal = Arc::new(OnSignal::new());
    let registry = BackgroundJobRegistry::new(1, kill_signal.clone(), Duration::from_millis(10));

    registry.shutdown();

    let result = registry.submit(|| {});
    assert!(result.is_err(), "submit after shutdown should fail");
}
