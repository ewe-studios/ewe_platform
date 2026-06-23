//! WHY: Verify the `IntervalCallbackList` and `IntervalRegistry` public APIs
//! for managing interval callbacks through REQUEUE/STOP tick states.
//!
//! WHAT: `IntervalCallbackList`, `IntervalRegistry`, `FnIntervalCallback`,
//! `TickState`.
//!
//! HOW: Register callbacks via the public API and assert length/value changes
//! after invocation.

use std::sync::{Arc, Mutex};

use foundation_wasm::{FnIntervalCallback, IntervalCallbackList, IntervalRegistry, TickState};

// -- IntervalCallbackList tests

#[test]
fn test_add_when_requeued() {
    let mut registry = IntervalCallbackList::new();

    let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let copy_value = value.clone();
    let handle = Box::new(FnIntervalCallback::from(move || {
        let mut item = copy_value.lock().unwrap();
        *item = 2;
        TickState::REQUEUE
    }));

    assert_eq!(registry.len(), 0);
    registry.add(handle);

    assert_eq!(registry.len(), 1);
    assert_eq!(*value.lock().unwrap(), 0);

    registry.call();

    assert_eq!(*value.lock().unwrap(), 2);

    assert_eq!(registry.len(), 1);
}

#[test]
fn test_add_when_stopping() {
    let mut registry = IntervalCallbackList::new();

    let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let copy_value = value.clone();
    let handle = Box::new(FnIntervalCallback::from(move || {
        let mut item = copy_value.lock().unwrap();
        *item = 2;
        TickState::STOP
    }));

    assert_eq!(registry.len(), 0);
    registry.add(handle);

    assert_eq!(registry.len(), 1);
    assert_eq!(*value.lock().unwrap(), 0);

    registry.call();

    assert_eq!(*value.lock().unwrap(), 2);

    assert_eq!(registry.len(), 0);
}

// -- IntervalRegistry tests

#[test]
fn test_interval_registry_add() {
    let mut registry = IntervalRegistry::new();

    let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let copy_value = value.clone();
    let handle = Box::new(FnIntervalCallback::from(move || {
        let mut item = copy_value.lock().unwrap();
        *item = 2;
        TickState::REQUEUE
    }));

    let id = registry.add(handle);

    assert_eq!(*value.lock().unwrap(), 0);

    let _ = registry.call(id);

    assert_eq!(*value.lock().unwrap(), 2);
}
