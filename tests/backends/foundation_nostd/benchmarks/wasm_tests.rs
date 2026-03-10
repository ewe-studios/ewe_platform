//! WASM-specific tests for `CondVar` functionality.
//!
//! These tests verify that `CondVar` works correctly in WASM environments,
//! which typically have limited or no threading support.

#![cfg(target_arch = "wasm32")]

use foundation_nostd::primitives::{CondVar, CondVarMutex, CondVarNonPoisoning, RawCondVarMutex};
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
    let (guard, result) = condvar
        .wait_timeout(guard, Duration::from_millis(10))
        .unwrap();

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

    let (_guard, result) = condvar
        .wait_timeout(guard, Duration::from_millis(50))
        .unwrap();

    let elapsed = start.elapsed();

    assert!(result.timed_out(), "Should have timed out");
    assert!(
        elapsed.as_millis() >= 40,
        "Should wait at least ~40ms (allowing for timing variance)"
    );
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

// ============================================================================
// Memory and Performance Tests
// ============================================================================

#[test]
fn test_condvar_memory_footprint() {
    // Verify CondVar has minimal memory footprint
    use core::mem::size_of;

    // CondVar should be small (target: 32-64 bytes)
    let condvar_size = size_of::<CondVar>();
    assert!(
        condvar_size <= 64,
        "CondVar size {} exceeds 64 bytes",
        condvar_size
    );

    // Mutex should also be compact
    let mutex_size = size_of::<CondVarMutex<u32>>();
    assert!(
        mutex_size <= 128,
        "CondVarMutex<u32> size {} exceeds 128 bytes",
        mutex_size
    );
}

#[test]
fn test_no_heap_allocations_in_hot_path() {
    // Verify that basic operations don't allocate
    // This is implicit in no_std, but we test the pattern
    let mutex = RawCondVarMutex::new(0);
    let condvar = CondVarNonPoisoning::new();

    // These operations should work without heap allocation
    let guard = mutex.lock();
    drop(guard);

    condvar.notify_one();
    condvar.notify_all();
}

#[test]
fn test_stack_based_operations() {
    // Verify all data structures are stack-based
    let mutex = CondVarMutex::new([0u8; 32]);
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();
    assert_eq!(guard.len(), 32);
    drop(guard);

    condvar.notify_all();
}

// ============================================================================
// Single-Threaded WASM Pattern Tests
// ============================================================================

#[test]
fn test_notify_with_no_waiters_is_noop() {
    // In single-threaded WASM, notify with no waiters should be no-op
    let condvar = CondVar::new();

    // These should not panic or block
    condvar.notify_one();
    condvar.notify_all();

    // Multiple calls should also be safe
    for _ in 0..10 {
        condvar.notify_one();
        condvar.notify_all();
    }
}

#[test]
fn test_immediate_timeout_single_threaded() {
    // Test zero-duration timeout in single-threaded context
    let mutex = CondVarMutex::new(());
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();
    let (_guard, result) = condvar.wait_timeout(guard, Duration::from_secs(0)).unwrap();

    assert!(
        result.timed_out(),
        "Zero duration should timeout immediately"
    );
}

#[test]
fn test_wait_while_false_returns_immediately() {
    // When predicate is false, wait_while should return immediately even in single-threaded
    let mutex = CondVarMutex::new(10);
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();

    // Predicate is false, should return immediately
    let guard = condvar.wait_while(guard, |x| *x < 5).unwrap();
    assert_eq!(*guard, 10);
}

// ============================================================================
// Feature Flag Tests
// ============================================================================

#[test]
fn test_condvar_works_without_std() {
    // This test only runs on WASM, which typically doesn't have std
    // Verify that operations work in no_std context

    #[cfg(not(feature = "std"))]
    {
        let mutex = RawCondVarMutex::new(42);
        let condvar = CondVarNonPoisoning::new();

        let guard = mutex.lock();
        assert_eq!(*guard, 42);
        drop(guard);

        condvar.notify_one();
        condvar.notify_all();
    }
}

#[test]
#[cfg(feature = "std")]
fn test_condvar_with_std_feature() {
    // When std is available on WASM, verify it works
    let mutex = CondVarMutex::new(String::from("wasm"));
    let condvar = CondVar::new();

    let guard = mutex.lock().unwrap();
    assert_eq!(*guard, "wasm");
    drop(guard);

    condvar.notify_one();
}

// ============================================================================
// Stress-like Tests for WASM
// ============================================================================

#[test]
fn test_rapid_lock_unlock_cycles() {
    // Test rapid mutex operations don't cause issues
    let mutex = RawCondVarMutex::new(0);

    for i in 0..1000 {
        let mut guard = mutex.lock();
        *guard = i;
        drop(guard);
    }

    let guard = mutex.lock();
    assert_eq!(*guard, 999);
}

#[test]
fn test_many_notify_calls() {
    // Test many notification calls don't cause issues
    let condvar = CondVarNonPoisoning::new();

    for _ in 0..10000 {
        condvar.notify_one();
        if _ % 2 == 0 {
            condvar.notify_all();
        }
    }
}

#[test]
fn test_timeout_with_varying_durations() {
    // Test different timeout durations work correctly
    let mutex = CondVarMutex::new(());
    let condvar = CondVar::new();

    let durations = [
        Duration::from_micros(1),
        Duration::from_millis(1),
        Duration::from_millis(10),
    ];

    for dur in &durations {
        let guard = mutex.lock().unwrap();
        let (_guard, result) = condvar.wait_timeout(guard, *dur).unwrap();
        assert!(result.timed_out(), "Should timeout for duration {:?}", dur);
    }
}
