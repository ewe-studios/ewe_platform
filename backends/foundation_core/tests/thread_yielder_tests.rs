//! Tests for ThreadYielder interruptibility
//!
//! These tests verify that ThreadYielder properly uses CondVar::wait_timeout
//! instead of park_timeout, allowing threads to be interrupted during shutdown.

#![cfg(feature = "multi")]

use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    multi::{initialize_pool, spawn},
    FnReady, NoSpawner, TaskIterator, TaskStatus,
};
use tracing_test::traced_test;

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

/// Task that completes immediately
struct ImmediateTask;

impl TaskIterator for ImmediateTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Some(TaskStatus::Ready(()))
    }
}

#[test]
#[traced_test]
#[serial]
fn shutdown_interrupts_delayed_tasks() {
    let start = Instant::now();

    // Initialize pool with seed=42, thread_num=10
    let guard = initialize_pool(42, Some(10));

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
#[traced_test]
#[serial]
fn multiple_delayed_tasks_shutdown_quickly() {
    let start = Instant::now();

    let guard = initialize_pool(42, Some(4));

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
/// When a thread is sleeping on a long sleeper deadline, spawning new work
/// should interrupt the sleep so the new work is picked up quickly.
#[test]
#[traced_test]
#[serial]
fn new_work_interrupts_sleep() {
    let start = Instant::now();
    let guard = initialize_pool(42, Some(4));

    // Spawn task with long delay - this will cause the thread to sleep for 30s
    spawn()
        .with_task(DelayedTask {
            delay: Duration::from_secs(30),
            ran: Arc::new(AtomicUsize::new(0)),
        })
        .with_resolver(Box::new(FnReady::new(|_, _| {})))
        .schedule()
        .expect("should schedule");

    // Let thread go to sleep
    std::thread::sleep(Duration::from_millis(100));

    // Spawn new immediate task - should wake sleeping thread and execute quickly
    let (tx, rx) = channel::<()>();

    spawn()
        .with_task(ImmediateTask)
        .with_resolver(Box::new(FnReady::new(move |_, _| {
            let _ = tx.send(());
        })))
        .schedule()
        .expect("should schedule");

    // Wait for the immediate task to complete
    // It should complete quickly (within 500ms) because the interrupt wakes the sleeping thread
    rx.recv_timeout(Duration::from_millis(500))
        .expect("Immediate task should complete quickly after interrupt");

    drop(guard);

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "Test took too long: {:?}",
        elapsed
    );
}
