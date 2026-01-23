//! WASM-specific tests for CondVar functionality.
//!
//! These tests verify that CondVar works correctly in WASM environments,
//! which typically have limited or no threading support.

#![cfg(target_arch = "wasm32")]

use foundation_nostd::primitives::{CondVar, CondVarMutex, RawCondVarMutex, CondVarNonPoisoning};
use std::time::Duration;

#[test]
fn test_condvar_basic_wasm() {
    // Basic test that CondVar can be created and used in WASM
    let mutex = CondVarMutex::new(0);
    let _condvar = CondVar::new();

    let guard = mutex.lock().unwrap();
    assert_eq!(*guard, 0);
}

#[test]
fn test_condvar_timeout_wasm() {
    // Test timeout functionality in WASM
    let mutex = CondVarMutex::new(false);
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();

    // This should timeout immediately since no one will notify
    let (guard, result) = condvar.wait_timeout(guard, Duration::from_millis(10)).unwrap();

    assert!(result.timed_out(), "Should have timed out");
    assert_eq!(*guard, false);
}

#[test]
fn test_condvar_wait_while_wasm() {
    // Test wait_while with a predicate
    let mutex = CondVarMutex::new(0);
    let condvar = CondVar::new();

    let mut guard = mutex.lock().unwrap();
    *guard = 5;

    // Condition is false, so this should return immediately
    let guard = condvar.wait_while(guard, |x| *x < 5).unwrap();
    assert_eq!(*guard, 5);
}

#[test]
fn test_condvar_non_poisoning_wasm() {
    // Test non-poisoning variant
    let mutex = RawCondVarMutex::new(42);
    let condvar = CondVarNonPoisoning::new();

    let guard = mutex.lock();
    assert_eq!(*guard, 42);
    drop(guard);

    // Notify operations should not panic
    condvar.notify_one();
    condvar.notify_all();
}

#[test]
fn test_condvar_multiple_waiters_wasm() {
    // Test notify_all in single-threaded WASM
    let mutex = CondVarMutex::new(vec![1, 2, 3]);
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();
    assert_eq!(guard.len(), 3);
    drop(guard);

    // In single-threaded WASM, notify_all should not panic
    condvar.notify_all();

    let guard = mutex.lock().unwrap();
    assert_eq!(*guard, vec![1, 2, 3]);
}

#[test]
fn test_condvar_timeout_accuracy_wasm() {
    // Verify that timeout durations work as expected in WASM
    let mutex = CondVarMutex::new(());
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();
    let start = std::time::Instant::now();

    let (_guard, result) = condvar.wait_timeout(guard, Duration::from_millis(50)).unwrap();

    let elapsed = start.elapsed();

    assert!(result.timed_out(), "Should have timed out");
    assert!(elapsed.as_millis() >= 40, "Should wait at least ~40ms (allowing for timing variance)");
}

#[test]
fn test_condvar_wait_timeout_while_wasm() {
    // Test wait_timeout_while in WASM
    let mutex = CondVarMutex::new(10);
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();

    // Condition is true initially, so should wait and timeout
    let (guard, result) = condvar
        .wait_timeout_while(guard, Duration::from_millis(10), |x| *x >= 10)
        .unwrap();

    assert!(result.timed_out());
    assert_eq!(*guard, 10);
}

#[test]
fn test_mutex_basic_operations_wasm() {
    // Test basic mutex operations in WASM
    let mutex = CondVarMutex::new(String::from("hello"));

    {
        let mut guard = mutex.lock().unwrap();
        guard.push_str(" world");
        assert_eq!(*guard, "hello world");
    }

    let guard = mutex.lock().unwrap();
    assert_eq!(*guard, "hello world");
}

#[test]
fn test_raw_mutex_no_poisoning_wasm() {
    // Verify RawCondVarMutex never poisons
    let mutex = RawCondVarMutex::new(100);

    let mut guard = mutex.lock();
    *guard += 50;
    assert_eq!(*guard, 150);
    drop(guard);

    let guard = mutex.lock();
    assert_eq!(*guard, 150);
}
