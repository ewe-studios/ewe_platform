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

// -- host runtime: callbacks that touch the interval registry

/// An interval callback that registers another interval and cancels itself
/// must not re-lock the registry it is being run from. It used to run under
/// `RECURRING_INTERVAL_CALLBACKS`' lock, so this deadlocked (or panicked).
#[test]
fn an_interval_callback_can_register_and_cancel_intervals() {
    use foundation_wasm::internal_api;
    use std::sync::mpsc;
    use std::time::Duration;

    let own_id: Arc<Mutex<Option<foundation_wasm::InternalPointer>>> = Arc::new(Mutex::new(None));
    let own = own_id.clone();
    let id = internal_api::register_interval(move || {
        let _other = internal_api::register_interval(|| TickState::STOP);
        if let Some(me) = *own.lock().unwrap() {
            internal_api::unregister_interval_callback(me);
        }
        TickState::STOP
    });
    *own_id.lock().unwrap() = Some(id);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(internal_api::run_interval_callback(id).is_ok());
    });
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)),
        Ok(true),
        "running the callback deadlocked or failed"
    );
}
