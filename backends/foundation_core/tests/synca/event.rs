//! Tests for `LockSignal`.

use std::{
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use foundation_core::synca::{LockState, Waker};

use foundation_core::synca::LockSignal;

#[test]
fn can_lock_signal() {
    let latch = Arc::new(LockSignal::new());

    let (sender, receiver) = mpsc::channel::<()>();

    let latch_clone = latch.clone();
    let handler = thread::spawn(move || {
        latch_clone.try_lock();
        sender.send(()).expect("should send");
        latch_clone.wait();
    });

    thread::sleep(Duration::from_millis(100));

    let _ = receiver.recv();
    assert_eq!(LockState::Locked, latch.probe());

    latch.signal_all();
    handler.join().expect("should safely join");

    assert_eq!(LockState::Free, latch.probe());
}

#[test]
fn test_lock_signal_new() {
    let signal = LockSignal::new();
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_lock_signal_default() {
    let signal = LockSignal::default();
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_try_lock_success() {
    let signal = LockSignal::new();
    assert!(signal.try_lock());
    assert_eq!(LockState::Locked, signal.probe());
}

#[test]
fn test_try_lock_already_locked() {
    let signal = LockSignal::new();
    assert!(signal.try_lock());
    // Second try_lock should return false since already locked
    assert!(!signal.try_lock());
    assert_eq!(LockState::Locked, signal.probe());
}

#[test]
fn test_lock_operation() {
    let signal = LockSignal::new();
    signal.lock();
    assert_eq!(LockState::Locked, signal.probe());

    // Locking again should be a no-op
    signal.lock();
    assert_eq!(LockState::Locked, signal.probe());
}

#[test]
fn test_probe_locked_true() {
    let signal = LockSignal::new();
    signal.lock();
    assert!(signal.probe_locked());
}

#[test]
fn test_probe_locked_false() {
    let signal = LockSignal::new();
    assert!(!signal.probe_locked());
}

#[test]
fn test_signal_one_releases_lock() {
    let signal = Arc::new(LockSignal::new());
    signal.lock();

    let (tx, rx) = mpsc::channel();
    let signal_clone = signal.clone();
    let handle = thread::spawn(move || {
        // Signal we're about to wait
        tx.send(()).unwrap();
        signal_clone.wait();
    });

    // Wait for thread to be ready
    rx.recv().unwrap();
    thread::sleep(Duration::from_millis(50));

    signal.signal_one();

    handle.join().expect("Thread should complete");
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_signal_all_releases_lock() {
    let signal = Arc::new(LockSignal::new());
    signal.lock();

    let (tx, rx) = mpsc::channel();
    let signal_clone = signal.clone();
    let handle = thread::spawn(move || {
        // Signal we're about to wait
        tx.send(()).unwrap();
        signal_clone.wait();
    });

    // Wait for thread to be ready
    rx.recv().unwrap();
    thread::sleep(Duration::from_millis(50));

    signal.signal_all();

    handle.join().expect("Thread should complete");
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_wait_when_free_returns_immediately() {
    let signal = LockSignal::new();
    // Should return immediately without blocking
    signal.wait();
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_lock_and_wait() {
    let signal = Arc::new(LockSignal::new());

    let (tx, rx) = mpsc::channel();
    let signal_clone = signal.clone();
    let handle = thread::spawn(move || {
        // Signal we're about to lock and wait
        tx.send(()).unwrap();
        signal_clone.lock_and_wait();
    });

    // Wait for thread to be ready
    rx.recv().unwrap();
    thread::sleep(Duration::from_millis(100));

    assert_eq!(LockState::Locked, signal.probe());

    signal.signal_all();
    handle.join().expect("Thread should complete");
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_multiple_waiters_signal_all() {
    let signal = Arc::new(LockSignal::new());
    signal.lock();

    let mut handles = vec![];

    // Spawn 3 waiting threads
    for _i in 0..3 {
        let signal_clone = signal.clone();
        handles.push(thread::spawn(move || {
            signal_clone.wait();
        }));
    }

    // Give threads time to enter wait state
    thread::sleep(Duration::from_millis(200));

    signal.signal_all();

    // Join all threads - they should all complete
    for handle in handles {
        handle.join().expect("Thread should complete");
    }

    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_waker_trait_implementation() {
    let signal = Arc::new(LockSignal::new());
    signal.lock();

    let (tx, rx) = mpsc::channel();
    let signal_clone = signal.clone();
    let handle = thread::spawn(move || {
        // Signal we're about to wait
        tx.send(()).unwrap();
        signal_clone.wait();
    });

    // Wait for thread to be ready
    rx.recv().unwrap();
    thread::sleep(Duration::from_millis(50));

    // Use Waker trait method
    signal.wake();

    handle.join().expect("Thread should complete");
    assert_eq!(LockState::Free, signal.probe());
}

#[test]
fn test_concurrent_probe_operations() {
    let signal = Arc::new(LockSignal::new());
    signal.lock();

    let mut handles = vec![];
    for _ in 0..5 {
        let signal_clone = signal.clone();
        handles.push(thread::spawn(move || {
            let state = signal_clone.probe();
            assert!(
                state == LockState::Locked
                    || state == LockState::Released
                    || state == LockState::Free
            );
        }));
    }

    thread::sleep(Duration::from_millis(50));
    signal.signal_all();

    for handle in handles {
        handle.join().expect("Thread should complete");
    }
}

#[test]
fn test_state_transitions() {
    let signal = LockSignal::new();

    // Free -> Locked
    assert_eq!(LockState::Free, signal.probe());
    signal.lock();
    assert_eq!(LockState::Locked, signal.probe());

    // Locked -> Released (via signal) -> Free (after wait completes)
    let signal = Arc::new(signal);
    let (tx, rx) = mpsc::channel();
    let signal_clone = signal.clone();
    let handle = thread::spawn(move || {
        tx.send(()).unwrap();
        signal_clone.wait();
    });

    rx.recv().unwrap();
    thread::sleep(Duration::from_millis(50));
    signal.signal_one();
    handle.join().expect("Thread should complete");

    // Released -> Free (after wait completes)
    assert_eq!(LockState::Free, signal.probe());
}
