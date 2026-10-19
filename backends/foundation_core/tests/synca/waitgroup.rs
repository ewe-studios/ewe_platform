//! Tests for `WaitGroup` and `WaitGroupGuard`.

use foundation_core::synca::WaitGroup;
use std::{sync::Arc, thread, time::Duration};

#[test]
fn test_waitgroup_wait_timeout_reaches_zero() {
    let wg = WaitGroup::new();
    wg.add(1);
    let wg2 = wg.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        wg2.done();
    });
    // Generous window: the worker finishes well before the deadline.
    assert!(wg.wait_timeout(Duration::from_secs(5)));
}

#[test]
fn test_waitgroup_wait_timeout_times_out() {
    let wg = WaitGroup::new();
    wg.add(1); // never completed
    let start = std::time::Instant::now();
    assert!(!wg.wait_timeout(Duration::from_millis(100)));
    assert!(start.elapsed() >= Duration::from_millis(100));
}

#[test]
fn test_waitgroup_wait_timeout_zero_count_returns_immediately() {
    let wg = WaitGroup::new();
    assert!(wg.wait_timeout(Duration::from_secs(5)));
}

#[test]
fn test_waitgroup_basic() {
    let wg = WaitGroup::new();
    wg.add(3);

    for _ in 0..3 {
        let wg = wg.clone();
        thread::spawn(move || {
            let _guard = wg.guard();
            thread::sleep(Duration::from_millis(10));
        });
    }

    wg.wait(); // Should block until all threads complete
}

#[test]
fn test_waitgroup_zero() {
    let wg = WaitGroup::new();
    // Should return immediately when count is 0
    wg.wait();
}

#[test]
fn test_waitgroup_add_done() {
    let wg = WaitGroup::new();
    wg.add(2);
    wg.done();
    wg.done();
    // Count should be 0 now
    wg.wait();
}

#[test]
fn test_waitgroup_guard_drop() {
    let wg = WaitGroup::new();
    wg.add(1);

    {
        let _guard = wg.guard();
        // Guard created, count still 1
    }
    // Guard dropped, done() called, count should be 0

    wg.wait(); // Should return immediately
}

#[test]
fn test_waitgroup_concurrent() {
    let wg = WaitGroup::new();
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    for i in 0..10 {
        let wg = wg.clone();
        let results = Arc::clone(&results);
        wg.add(1);
        thread::spawn(move || {
            let _guard = wg.guard();
            thread::sleep(Duration::from_millis(i as u64));
            results.lock().unwrap().push(i);
        });
    }

    wg.wait();

    let results = results.lock().unwrap();
    assert_eq!(results.len(), 10);
}
