//! Tests for ThreadYielder interruptibility
//!
//! These tests verify that ThreadYielder properly uses CondVar::wait_timeout
//! instead of park_timeout, allowing threads to be interrupted during shutdown.

#![cfg(feature = "multi")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    multi::{initialize_pool, spawn},
    FnReady, NoSpawner, TaskIterator, TaskStatus,
};

/// Task that returns Delayed with a specific duration
struct DelayedTask {
    delay: Duration,
    ran: Arc<AtomicUsize>,
}

impl TaskIterator for DelayedTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        self.ran.fetch_add(1, Ordering::Relaxed);
        Some(TaskStatus::Delayed(self.delay))
    }
}

#[test]
fn shutdown_interrupts_delayed_tasks() {
    let start = Instant::now();

    // Initialize pool with 2 threads
    let guard = initialize_pool(2, Some(42));

    // Spawn task with 10s delay
    spawn()
        .with_task(DelayedTask {
            delay: Duration::from_secs(10),
            ran: Arc::new(AtomicUsize::new(0)),
        })
        .with_resolver(Box::new(FnReady::new(|_, _| {})))
        .schedule()
        .expect("should schedule");

    // Let thread start sleeping
    std::thread::sleep(Duration::from_millis(50));

    // Drop guard triggers shutdown
    drop(guard);

    let elapsed = start.elapsed();

    // Should complete quickly (< 1s), not wait 10s
    assert!(
        elapsed < Duration::from_secs(1),
        "Shutdown took too long: {:?}",
        elapsed
    );
}

#[test]
fn multiple_delayed_tasks_shutdown_quickly() {
    let start = Instant::now();

    let guard = initialize_pool(4, Some(42));

    // Spawn multiple tasks with long delays
    for _ in 0..10 {
        spawn()
            .with_task(DelayedTask {
                delay: Duration::from_secs(30),
                ran: Arc::new(AtomicUsize::new(0)),
            })
            .with_resolver(Box::new(FnReady::new(|_, _| {})))
            .schedule()
            .expect("should schedule");
    }

    std::thread::sleep(Duration::from_millis(100));

    // Shutdown should interrupt all
    drop(guard);

    let elapsed = start.elapsed();

    // Should still complete quickly
    assert!(
        elapsed < Duration::from_secs(2),
        "Shutdown with multiple delayed tasks took too long: {:?}",
        elapsed
    );
}

/// Test that new work interrupts sleeping threads.
/// This requires sleeper-aware yielding from Feature 03.
#[test]
#[ignore = "requires Feature 03: sleeper-aware yielding"]
fn new_work_interrupts_sleep() {
    // This test will be enabled when Feature 03 is implemented.
    // It verifies that when a thread is sleeping (waiting on a sleeper),
    // new work arrival notifies the CondVar to wake it up early.
}
