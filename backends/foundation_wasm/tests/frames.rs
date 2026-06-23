//! WHY: Verify the `FrameCallbackList` public API correctly manages callbacks
//! through REQUEUE and STOP tick states.
//!
//! WHAT: `FrameCallbackList`, `FnFrameCallback`, `TickState`.
//!
//! HOW: Register callbacks via the public API and assert length/value changes
//! after `call()`.

use std::sync::{Arc, Mutex};

use foundation_wasm::{FnFrameCallback, FrameCallbackList, TickState};

#[test]
fn test_add_when_requeued() {
    let mut registry = FrameCallbackList::new();

    let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let copy_value = value.clone();
    let handle = Box::new(FnFrameCallback::from(move |value| {
        let mut item = copy_value.lock().unwrap();
        *item = value as usize;
        TickState::REQUEUE
    }));

    assert_eq!(registry.len(), 0);
    registry.add(handle);

    assert_eq!(registry.len(), 1);
    assert_eq!(*value.lock().unwrap(), 0);

    registry.call(2.0);

    assert_eq!(*value.lock().unwrap(), 2);

    assert_eq!(registry.len(), 1);
}

#[test]
fn test_add_when_stopping() {
    let mut registry = FrameCallbackList::new();

    let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let copy_value = value.clone();
    let handle = Box::new(FnFrameCallback::from(move |value| {
        let mut item = copy_value.lock().unwrap();
        *item = value as usize;
        TickState::STOP
    }));

    assert_eq!(registry.len(), 0);
    registry.add(handle);

    assert_eq!(registry.len(), 1);
    assert_eq!(*value.lock().unwrap(), 0);

    registry.call(2.0);

    assert_eq!(*value.lock().unwrap(), 2);

    assert_eq!(registry.len(), 0);
}
