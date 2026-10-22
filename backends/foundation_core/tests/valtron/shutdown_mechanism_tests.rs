//! Tests for sleeper-aware yielding in Feature 03
//!
//! These tests verify that the executor properly uses sleeper deadlines
//! to determine yield duration when no work is available.

#![cfg(feature = "multi")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use foundation_core::valtron::{Ctx, 
    multi::spawn,
    valtron_test,
    FnReady, TaskIterator, TaskStatus,
};
/// Task that returns Ready a limited number of times then completes
struct FiniteTask {
    remaining: usize,
    value: usize,
}

impl TaskIterator for FiniteTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(TaskStatus::Ready(self.value))
    }
}

/// WHY: Validates basic task execution with sleeper-aware yielding
/// WHAT: Tasks should complete normally when sleeper-aware yielding is active
/// HOW: Spawn tasks and verify they complete within expected time
#[valtron_test(seed = 42, threads = 3)]
fn basic_execution_with_sleeper_aware_yielding() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    // Spawn a simple task
    spawn()
        .with_task(FiniteTask {
            remaining: 5,
            value: 42,
        })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(val) = item {
                counter_clone.fetch_add(val, Ordering::SeqCst);
            }
        })))
        .schedule()
        .expect("should schedule");

    // Wait for completion
    let start = Instant::now();
    while counter.load(Ordering::SeqCst) < 210 && start.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(5));
    }

    // 5 * 42 = 210
    assert_eq!(counter.load(Ordering::SeqCst), 210);
}

/// WHY: Validates multiple tasks complete with sleeper-aware yielding
/// WHAT: Multiple concurrent tasks should all complete
/// HOW: Spawn multiple tasks and verify all complete
#[valtron_test(seed = 42, threads = 3)]
fn multiple_tasks_complete_with_sleeper_aware_yielding() {
    let results = Arc::new(Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    // Spawn multiple tasks
    for i in 0..5 {
        let results_inner = Arc::clone(&results_clone);
        spawn()
            .with_task(FiniteTask {
                remaining: 1,
                value: i,
            })
            .with_resolver(Box::new(FnReady::new(move |item, _| {
                if let TaskStatus::Ready(val) = item {
                    results_inner.lock().unwrap().push(val);
                }
            })))
            .schedule()
            .expect("should schedule");
    }

    // Wait for all to complete
    let start = Instant::now();
    while results.lock().unwrap().len() < 5 && start.elapsed() < Duration::from_secs(30) {
        thread::sleep(Duration::from_millis(100));
    }

    let final_results = results.lock().unwrap();
    assert_eq!(final_results.len(), 5);
}

/// A task whose single step blocks its worker until released, like a task
/// stuck in a blocking syscall or a deadlock.
struct BlockingTask {
    started: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
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

/// WHY: `PoolGuard::shutdown` waits for every worker to reach its kill check,
/// so one worker blocked inside a task makes it wait forever and keeps the
/// process-wide pool gate shut. `shutdown_within` is the bounded teardown the
/// `#[valtron_test(timeout = ..)]` harness relies on.
///
/// WHAT: with a worker blocked, `shutdown_within` returns `false` within its
/// deadline, and dropping the guard releases the gate so a new pool can start
/// and run work.
///
/// HOW: block one worker with `BlockingTask`, tear down with a 200 ms deadline,
/// then bring up a second pool on another thread and require it to finish a
/// future within 10 s (a hang there means the gate was not released).
#[test]
fn shutdown_within_detaches_a_blocked_worker_and_releases_the_pool_gate() {
    use foundation_core::valtron::{block_on_future, initialize_pool, send};

    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();

    let guard = initialize_pool(7, Some(3));
    send(BlockingTask {
        started: started_tx,
        release: release_rx,
    })
    .expect("schedule the blocking task");
    started_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the blocking task should start on a worker");

    let start = Instant::now();
    let stopped = guard.shutdown_within(Duration::from_millis(200));
    let took = start.elapsed();
    drop(guard);

    assert!(!stopped, "a blocked worker cannot stop, so shutdown_within must report it");
    assert!(
        took < Duration::from_secs(5),
        "shutdown_within must return near its 200 ms deadline, took {took:?}"
    );

    // The gate must be free: a fresh pool comes up and runs a future.
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let guard = initialize_pool(8, Some(3));
        let value = block_on_future(async { 41 + 1 });
        drop(guard);
        let _ = done_tx.send(value);
    });
    let value = done_rx.recv_timeout(Duration::from_secs(10));

    // Let the detached worker finish now that the assertions are in.
    let _ = release_tx.send(());

    assert_eq!(
        value.expect("a new pool must start after shutdown_within (pool gate released)"),
        42
    );
}
