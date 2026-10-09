//! WHY: Verify the `ScheduleRegistry` public API for adding and calling
//! scheduled tasks.
//!
//! WHAT: `FnDoTask`, `ScheduleRegistry`.
//!
//! HOW: Register a task callback, invoke it by ID, and assert the side-effect.

use std::sync::{Arc, Mutex};

use foundation_wasm::{FnDoTask, ScheduleRegistry};

#[test]
fn test_add() {
    let mut registry = ScheduleRegistry::new();

    let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let copy_value = value.clone();
    let handle = Box::new(FnDoTask::from(move || {
        let mut item = copy_value.lock().unwrap();
        *item = 2;
    }));

    let id = registry.add(handle);

    assert_eq!(*value.lock().unwrap(), 0);

    let _ = registry.call(id);

    assert_eq!(*value.lock().unwrap(), 2);
}
