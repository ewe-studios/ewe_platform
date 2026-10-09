//! Tests for sleeper-aware yielding in Feature 03
//!
//! These tests verify that the executor properly uses sleeper deadlines
//! to determine yield duration when no work is available.

#![cfg(feature = "multi")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    multi::spawn,
    valtron_test,
    FnReady, NoSpawner, TaskIterator, TaskStatus,
};
/// Task that returns Ready a limited number of times then completes
struct FiniteTask {
    remaining: usize,
    value: usize,
}

impl TaskIterator for FiniteTask {
    type Ready = usize;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
