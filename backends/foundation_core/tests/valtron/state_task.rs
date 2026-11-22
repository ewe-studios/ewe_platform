//! Feature 06: the `#[task]` state-machine contract on the NATIVE sync boundary.
//!
//! The cases that use only `TaskStatus`, the generated struct and the executor
//! live in `state_task_wasm.rs`, which also runs on a JS-hosted wasm build. This
//! file keeps the cases that need the blocking sync-boundary helper `sync_one`,
//! which does not exist on JS-hosted wasm (see `tests/valtron/mod.rs`).

use foundation_core::valtron::{sync_one, valtron_test};

// The two generated machines and the `YieldOnce` helper live with their
// dual-target cases; these cases only drive them through `sync_one`.
use super::state_task_wasm::{async_count_down, count_down};

/// The generated struct schedules on the real executor like any other task and
/// collects its single `Ready` value.
#[valtron_test]
fn task_macro_sync_schedules_on_pool() {
    let results = sync_one(count_down::new(3)).expect("engine must be initialized");
    assert_eq!(results, vec![1]);
}

/// The async generated struct schedules on the pool, parks on `Depends`, and
/// resumes when woken — never busy-polling.
#[valtron_test]
fn task_macro_async_schedules_on_pool() {
    let results = sync_one(async_count_down::new(3)).expect("engine must be initialized");
    assert_eq!(results, vec![1]);
}
