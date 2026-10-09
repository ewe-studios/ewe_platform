//! Integration tests for valtron's cooperative sleep (`sleep`, `sleep_async`).

use std::time::{Duration, Instant};

use foundation_core::valtron::{sleep, sleep_async, valtron_test, SleepingTask, TaskIterator};

// ── SleepingTask unit tests (no executor needed) ──────────────────────

#[test]
fn sleeping_task_elapsed_returns_none() {
    let mut t = SleepingTask::sleep(Duration::ZERO);
    assert!(t.next_status().is_none(), "already-elapsed sleep should be done");
}

#[test]
fn sleeping_task_future_yields_delayed() {
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut t = SleepingTask::new(deadline);
    let status = t.next_status();
    assert!(
        matches!(status, Some(foundation_core::valtron::TaskStatus::Delayed(_))),
        "future deadline should yield Delayed"
    );
}

#[test]
fn sleeping_task_eventually_completes() {
    let mut t = SleepingTask::sleep(Duration::from_millis(1));
    // Drive until done — the Delayed loop should terminate
    for _ in 0..10 {
        match t.next_status() {
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
