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

use foundation_compact::Instant;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    single::{initialize_pool, run_until_complete, spawn},
    wasm::{JSThreadYielder, WaitStatus},
    FnReady, ProcessController, TaskIterator, TaskStatus,
};

use foundation_context::Ctx;
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
    // Warm up: the first call on a fresh instance pays one-off costs (JIT,
    // first host-time read) that can exceed the bound on a busy runner.
    let _ = yielder.yield_for(Duration::from_millis(100));
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

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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
        .as_scheduled()
        .spawn()
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
            .as_scheduled()
            .spawn()
            .expect("should schedule task");
        let _ = i; // suppress unused variable warning
    }

    run_until_complete();

    let elapsed = start.elapsed();

    assert_eq!(
        done_count.load(Ordering::Relaxed),
        2,
        "Both tasks should complete"
    );
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
        .as_scheduled()
        .spawn()
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

// ---------------------------------------------------------------------------
// Spec 68 / spec 65 feature 06 T5: event-driven park → wake → resume on the
// wasm-bindgen backend (the one `cf-cloud-provider` runs on).

use std::cell::RefCell;
use std::rc::Rc;

use foundation_core::valtron::{WakeCallback, WakeSignal};

/// Resolve after `ms` on the JS event loop (`setTimeout` looked up on the global
/// object, so it works in Deno, browsers and Workers alike).
async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout: js_sys::Function =
            js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("setTimeout"))
                .expect("global setTimeout")
                .unchecked_into();
        set_timeout
            .call2(&global, &resolve, &wasm_bindgen::JsValue::from(ms))
            .expect("setTimeout call");
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .expect("timer promise");
}

/// Parks on a [`WakeSignal`] on its first poll, then emits one value.
struct ParkOnceTask {
    polls: Rc<AtomicUsize>,
    callback_slot: Rc<RefCell<Option<WakeCallback>>>,
    step: u8,
}

impl TaskIterator for ParkOnceTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::Relaxed);
        let step = self.step;
        self.step = self.step.saturating_add(1);
        match step {
            0 => {
                let signal = WakeSignal::new();
                *self.callback_slot.borrow_mut() = Some(signal.callback());
                Some(TaskStatus::Depends(Arc::new(signal)))
            }
            1 => Some(TaskStatus::Ready(7)),
            _ => None,
        }
    }
}

/// A task parked on `Depends(WakeSignal)`:
/// - `run_until_complete` returns to JS once it is parked (no spin);
/// - it is not re-polled while the event loop turns (no timer polling);
/// - firing the wake from JS-side async work (`spawn_local` after a timer, as a
///   fetch completing would) resumes the executor via the microtask resume and
///   the task completes.
#[wasm_bindgen_test]
async fn depends_task_parks_until_wake_signal_fires() {
    initialize_pool(42);

    let polls = Rc::new(AtomicUsize::new(0));
    let callback_slot: Rc<RefCell<Option<WakeCallback>>> = Rc::new(RefCell::new(None));
    let value = Arc::new(AtomicUsize::new(0));

    let value_clone = Arc::clone(&value);
    spawn()
        .with_task(ParkOnceTask {
            polls: Rc::clone(&polls),
            callback_slot: Rc::clone(&callback_slot),
            step: 0,
        })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(v) = item {
                value_clone.store(v, Ordering::Relaxed);
            }
        })))
        .as_scheduled()
        .spawn()
        .expect("should schedule");

    run_until_complete();
    assert_eq!(polls.load(Ordering::Relaxed), 1, "polled once, then parked");

    // Let the event loop turn: a parked task must not be re-polled.
    sleep_ms(20).await;
    assert_eq!(
        polls.load(Ordering::Relaxed),
        1,
        "a parked task must not be re-polled before its signal fires"
    );

    // Fire the wake from JS-side async work, like a fetch resolving.
    let slot = Rc::clone(&callback_slot);
    wasm_bindgen_futures::spawn_local(async move {
        sleep_ms(5).await;
        let wake = slot
            .borrow_mut()
            .take()
            .expect("the task registered its wake");
        wake();
    });

    for _ in 0..200 {
        if value.load(Ordering::Relaxed) == 7 {
            break;
        }
        sleep_ms(1).await;
    }
    assert_eq!(
        value.load(Ordering::Relaxed),
        7,
        "resumed and completed after the wake"
    );
}

/// Parks on a [`BoolSignal`](foundation_core::valtron::BoolSignal) until it is
/// set, then emits one value.
struct ParkOnBoolTask {
    signal: foundation_core::valtron::BoolSignal,
    step: u8,
}

impl TaskIterator for ParkOnBoolTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        use foundation_core::valtron::EventReadiness;
        match self.step {
            0 if !self.signal.is_ready(None) => {
                Some(TaskStatus::Depends(Arc::new(self.signal.clone())))
            }
            0 => {
                self.step = 1;
                Some(TaskStatus::Ready(9))
            }
            _ => None,
        }
    }
}

/// A task parked on a `BoolSignal` resumes when JS-side async work sets it.
/// The executor hands control back to the event loop while every task is
/// parked; `BoolSignal::set` asks it to run again. A raw store on the atomic
/// did not, so the task was never re-polled.
#[wasm_bindgen_test]
async fn bool_signal_set_from_js_resumes_a_parked_task() {
    initialize_pool(43);

    let signal = foundation_core::valtron::BoolSignal::new(false);
    let value = Arc::new(AtomicUsize::new(0));
    let value_clone = Arc::clone(&value);
    spawn()
        .with_task(ParkOnBoolTask {
            signal: signal.clone(),
            step: 0,
        })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(v) = item {
                value_clone.store(v, Ordering::Relaxed);
            }
        })))
        .as_scheduled()
        .spawn()
        .expect("should schedule");

    run_until_complete();
    assert_eq!(
        value.load(Ordering::Relaxed),
        0,
        "parked until the signal is set"
    );

    wasm_bindgen_futures::spawn_local(async move {
        sleep_ms(5).await;
        signal.set(true);
    });

    for _ in 0..200 {
        if value.load(Ordering::Relaxed) == 9 {
            break;
        }
        sleep_ms(1).await;
    }
    assert_eq!(
        value.load(Ordering::Relaxed),
        9,
        "resumed after BoolSignal::set"
    );
}

/// Calls `run_until_complete` from inside its own `next_status` — what a sync
/// "block on" helper reached from a task does — then finishes.
struct NestedRunTask {
    step: u8,
}

impl TaskIterator for NestedRunTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.step {
            0 => {
                self.step = 1;
                // Nothing else is scheduled: this nested run must return, not
                // spin on the slot of the very task that called it.
                run_until_complete();
                Some(TaskStatus::Ready(11))
            }
            _ => None,
        }
    }
}

/// A `run_until` nested inside a task returns when only its callers are left.
/// The polled task's slot counts as active, so the nested loop used to see
/// "work to do" for ever and freeze the isolate without yielding to JS.
#[wasm_bindgen_test]
fn nested_run_until_returns_instead_of_spinning() {
    initialize_pool(44);

    let value = Arc::new(AtomicUsize::new(0));
    let value_clone = Arc::clone(&value);
    spawn()
        .with_task(NestedRunTask { step: 0 })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(v) = item {
                value_clone.store(v, Ordering::Relaxed);
            }
        })))
        .as_scheduled()
        .spawn()
        .expect("should schedule");

    run_until_complete();
    assert_eq!(
        value.load(Ordering::Relaxed),
        11,
        "the nested run returned and the task finished"
    );
}

// ---------------------------------------------------------------------------
// Spec 70 feature 80 E5: the per-task `ReadinessWaker` design at runtime on the
// wasm-bindgen backend. The scenarios live in `wasm_wake_support`; this file
// supplies the host timer (a JS `setTimeout`) and the wait.

use super::wasm_wake_support::{self as wake, Scenario};

/// Run `callback` after `delay_ms` on the JS event loop, as a host event (a fetch
/// completing, a message arriving) would.
fn host_after(delay_ms: f64, callback: Box<dyn FnOnce()>) {
    wasm_bindgen_futures::spawn_local(async move {
        sleep_ms(delay_ms as i32).await;
        callback();
    });
}

/// Wait for the scenario's task to end, or fail after a few seconds with its
/// name: a lost wake leaves the task parked for ever, and the test must fail,
/// not hang.
async fn finish(scenario: Scenario) {
    let start = Instant::now();
    while !scenario.is_done() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "{}: the parked task never ran again (a lost wake)",
            scenario.name()
        );
        sleep_ms(1).await;
    }
    scenario.verify();
}

/// A task parked on a `ResumeSignal` that a JS timer sets 50 ms later resumes on
/// time. See `wasm_wake_support::resume_signal_park`.
#[wasm_bindgen_test]
async fn resume_signal_set_by_host_timer_wakes_parked_task() {
    finish(wake::resume_signal_park(host_after)).await;
}

/// An `EventQueue` push from a JS callback wakes a consumer parked on
/// `queue.readiness()`. See `wasm_wake_support::queue_push_park`.
#[wasm_bindgen_test]
async fn event_queue_push_from_host_wakes_parked_consumer() {
    finish(wake::queue_push_park(host_after)).await;
}

/// An `EventQueue` close from a JS callback wakes a parked consumer. See
/// `wasm_wake_support::queue_close_park`.
#[wasm_bindgen_test]
async fn event_queue_close_from_host_wakes_parked_consumer() {
    finish(wake::queue_close_park(host_after)).await;
}

/// A deadline-only park resumes at its deadline without blocking the event loop.
/// See `wasm_wake_support::deadline_only_park`.
#[wasm_bindgen_test]
async fn deadline_only_park_resumes_at_its_deadline() {
    finish(wake::deadline_only_park(host_after)).await;
}

/// An `AnyReadiness` over two children resumes when the first child is ready.
/// See `wasm_wake_support::any_readiness_park`.
#[wasm_bindgen_test]
async fn any_readiness_resumes_when_a_child_is_ready() {
    finish(wake::any_readiness_park(host_after)).await;
}

/// A valtron `sleep` ends at its deadline. See `wasm_wake_support::sleep_task`.
#[wasm_bindgen_test]
async fn sleep_task_ends_at_its_deadline() {
    finish(wake::sleep_task(host_after)).await;
}

/// A `TimeoutTask` ends at its timeout. See `wasm_wake_support::timeout_task`.
#[wasm_bindgen_test]
async fn timeout_task_ends_at_its_timeout() {
    finish(wake::timeout_task(host_after)).await;
}

/// `WaitGroup::wait_timeout` on an idle group does not panic on the clock. See
/// `wasm_wake_support::waitgroup_idle_wait_timeout`.
#[wasm_bindgen_test]
fn waitgroup_idle_wait_timeout_reads_the_clock() {
    wake::waitgroup_idle_wait_timeout();
}
