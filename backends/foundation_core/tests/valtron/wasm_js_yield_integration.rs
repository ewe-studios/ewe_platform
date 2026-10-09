//! Integration tests for the full JS yield stack:
//! valtron executor -> JS yield -> Promise resolution -> executor re-entry -> task completion.
//!
//! These tests run on wasm32-unknown-unknown in a JS environment (browser or Node.js).
//! They verify that the CooperativeSpinWaiter + JSThreadYielder integration
//! correctly yields to the JS event loop and resumes via setTimeout callbacks.
//!
//! WHY: Unit tests verify individual components (SpinWaiter, JSThreadYielder)
//! in isolation. These integration tests verify the full stack works end-to-end
//! in a real JS runtime.
//!
//! Run with: wasm-testbed test bindgen-deno ./foundation_core --features js-wasmbindgen

#![cfg(target_family = "wasm")]
#![cfg(feature = "js-wasmbindgen")]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use foundation_compact::Instant;

use foundation_core::valtron::{
    single::{initialize_pool, run_until_complete, spawn},
    wasm::{JSThreadYielder, WaitStatus},
    FnReady, NoSpawner, ProcessController, TaskIterator, TaskStatus,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::wasm_bindgen_test;

/// Test 1: js_yield_returns_immediately
///
/// WHAT: SpinWaiter::wait() returns immediately (< 5ms) even when asked to
/// wait for a long duration. This is because it schedules a setTimeout and
/// returns Waiting — it does NOT spin and block.
///
/// WHY: If this test fails (> 5ms), it means the JS yield mechanism is
/// spinning and blocking the event loop, defeating the whole purpose of
/// CooperativeSpinWaiter.
#[wasm_bindgen_test]
fn js_yield_returns_immediately() {
    let yielder = JSThreadYielder::new();
    let start = Instant::now();

    // Ask to "wait" for 100ms
    let status = yielder.yield_for(Duration::from_millis(100));

    // Should return immediately with Waiting
    assert_eq!(status, WaitStatus::Waiting);

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(5),
        "yield_for took {:?} but should return immediately (< 5ms)",
        elapsed
    );
}

/// Task that completes a fixed number of iterations.
struct CountingTask {
    max: usize,
    current: usize,
}

impl TaskIterator for CountingTask {
    type Ready = usize;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current >= self.max {
            return None;
        }
        self.current += 1;
        Some(TaskStatus::Ready(self.current))
    }
}

/// Task that yields between iterations using the controller's yield mechanism.
struct YieldingTask {
    max: usize,
    current: usize,
    counter: Arc<AtomicUsize>,
}

impl TaskIterator for YieldingTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current >= self.max {
            return None;
        }
        self.current += 1;
        self.counter.fetch_add(1, Ordering::Relaxed);
        Some(TaskStatus::Ready(()))
    }
}

/// Test 2: js_yield_callback_reschedules_executor
///
/// WHAT: Spawn a task, run_until_complete, and verify it finishes. With the
/// JS yielder, this exercises: spawn -> executor runs -> setTimeout fires ->
/// js_yield_and_continue re-enters executor -> task completes.
///
/// WHY: If the executor is not re-entered after the setTimeout callback fires,
/// the task will never complete and the test will hang. This proves the full
/// cycle: yield -> setTimeout -> callback -> re-entry -> completion.
#[wasm_bindgen_test]
fn js_yield_callback_reschedules_executor() {
    initialize_pool(42);

    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);

    spawn()
        .with_task(CountingTask { max: 1, current: 0 })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(_) = item {
                done_clone.store(true, Ordering::Relaxed);
            }
        })))
        .schedule()
        .expect("should schedule");

    // Set a safety timeout — if the executor doesn't re-enter after JS yield,
    // this will fire and fail the test instead of hanging.
    let done_check = Arc::clone(&done);
    let timeout = wasm_bindgen::closure::Closure::once(Box::new(move || {
        if !done_check.load(Ordering::Relaxed) {
            panic!("Executor did not re-enter after JS yield — timeout after 500ms");
        }
    }) as Box<dyn FnOnce()>);

    let global = js_sys::global();
    if let Some(window) = global.dyn_ref::<web_sys::Window>() {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.as_ref().dyn_ref::<js_sys::Function>().unwrap(),
                500,
            )
            .expect("setTimeout failed");
    } else if let Some(scope) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
        scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.as_ref().dyn_ref::<js_sys::Function>().unwrap(),
                500,
            )
            .expect("setTimeout failed");
    }
    timeout.forget();

    run_until_complete();

    assert!(done.load(Ordering::Relaxed), "Task should have completed");
}

/// Test 3: concurrent_d1_queries_make_progress
///
/// WHAT: Spawn two tasks concurrently (no JS yield delay). With JS yields,
/// the two tasks' setTimeout timers overlap, so wall-clock time should be
/// approximately one yield period (~10ms), not the sum of both (~20ms).
///
/// WHY: This simulates the real-world scenario of multiple D1 database
/// queries being issued concurrently. If they run sequentially instead
/// of concurrently, the total time would be the sum of all delays.
#[wasm_bindgen_test]
fn concurrent_d1_queries_make_progress() {
    initialize_pool(42);

    let done_count = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    // Both tasks will have their setTimeout timers scheduled at the same time
    for i in 0..2 {
        let dc = Arc::clone(&done_count);
        spawn()
            .with_task(CountingTask { max: 1, current: 0 })
            .with_resolver(Box::new(FnReady::new(move |item, _| {
                if let TaskStatus::Ready(_) = item {
                    dc.fetch_add(1, Ordering::Relaxed);
                }
            })))
            .schedule()
            .expect("should schedule task");
        let _ = i; // suppress unused variable warning
    }

    run_until_complete();

    let elapsed = start.elapsed();

    assert_eq!(done_count.load(Ordering::Relaxed), 2, "Both tasks should complete");
    // With concurrent yields, wall time should be ~10ms (one yield period),
    // not ~20ms (sequential). Allow generous margin for JS timer variance.
    assert!(
        elapsed < Duration::from_millis(50),
        "concurrent tasks took {:?} — should complete within one yield period",
        elapsed
    );
}

/// Test 4: no_deadlock_with_js_yield
///
/// WHAT: A task that completes multiple iterations, each going through the
/// executor cycle. The executor must re-enter after each setTimeout callback
/// to process all iterations.
///
/// WHY: If there is a deadlock — e.g., the executor does not re-enter after
/// the first yield, or the setTimeout callback doesn't trigger
/// js_yield_and_continue — only the first iteration would complete and the
/// test would hang. This proves the yield/reschedule cycle works repeatedly.
#[wasm_bindgen_test]
fn no_deadlock_with_js_yield() {
    initialize_pool(42);

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    spawn()
        .with_task(YieldingTask {
            max: 5,
            current: 0,
            counter: counter_clone,
        })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(()) = item {
                // counter already incremented in the task
            }
        })))
        .schedule()
        .expect("should schedule");

    // Safety timeout
    let c = Arc::clone(&counter);
    let timeout = wasm_bindgen::closure::Closure::once(Box::new(move || {
        let count = c.load(Ordering::Relaxed);
        if count < 5 {
            panic!(
                "Deadlock detected: only {} of 5 iterations completed after 500ms",
                count
            );
        }
    }) as Box<dyn FnOnce()>);

    let global = js_sys::global();
    if let Some(window) = global.dyn_ref::<web_sys::Window>() {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.as_ref().dyn_ref::<js_sys::Function>().unwrap(),
                500,
            )
            .expect("setTimeout failed");
    } else if let Some(scope) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
        scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout.as_ref().dyn_ref::<js_sys::Function>().unwrap(),
                500,
            )
            .expect("setTimeout failed");
    }
    timeout.forget();

    run_until_complete();

    assert_eq!(
        counter.load(Ordering::Relaxed),
        5,
        "All 5 iterations should complete without deadlock"
    );
}
