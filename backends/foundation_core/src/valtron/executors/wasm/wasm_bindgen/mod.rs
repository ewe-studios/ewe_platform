//! wasm-bindgen + web-sys backend for JS event loop yielding and JS stream bridging.
//!
//! Uses `window.setTimeout` directly for timer scheduling.

pub mod js_stream;

use crate::valtron::{ProgressIndicator, State};
use foundation_nostd::primitives::cooperative_spin_waiter::{CooperativeSpinWaiter, WaitStatus};
use std::time;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

/// Module-level checker used by both the executor's `run_until` and the
/// setTimeout callback's re-invocation. Returns true when the executor
/// should stop and yield back to JS.
///
/// NOTE: `Wait` and `SpinWait` are deliberately NOT stop states. They are
/// "yield" states: `run_until` handles them in its own arms (scheduling a
/// fresh `setTimeout` before returning). If the checker stopped on them, the
/// setTimeout re-invocation would break on the very first `Wait` without
/// rescheduling, and a task parked on in-flight async work (e.g. the wasm lazy
/// SSE fetch) would never be re-polled — freezing the isolate.
fn js_should_stop(signal: ProgressIndicator) -> bool {
    matches!(
        signal,
        ProgressIndicator::NoWork | ProgressIndicator::CanProgress(Some(State::Reschedule))
    )
}

/// Module-level function called from the setTimeout closure.
/// After the timer fires and the resumed flag is set, this re-invokes
/// the valtron executor to continue processing. A no-op when this thread never
/// initialized a pool (a yield outside any executor still queues the timer).
fn js_yield_and_continue() {
    if crate::valtron::single::is_initialized() {
        crate::valtron::single::run_until(js_should_stop);
    }
}

thread_local! {
    /// Set while a resume microtask is queued, so a burst of wakes (one per SSE
    /// chunk, say) queues a single executor run instead of one per wake.
    static RESUME_QUEUED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Ask the JS event loop to run the valtron executor again — the event-driven
/// counterpart to the `setTimeout` yield.
///
/// WHY: a task parked on a readiness signal (`TaskStatus::Depends`) has nothing
/// to poll; it becomes runnable only when some JS-side async work (a `fetch`, a
/// `ReadableStream` read) completes. Instead of re-checking on a timer, the code
/// that observes that completion calls this, and the executor runs exactly once
/// per burst of wakes.
///
/// HOW: queues a microtask via `wasm_bindgen_futures::spawn_local` (the same
/// queue wasm-bindgen drives Rust futures on), so it runs as soon as the current
/// JS turn finishes — never re-entrantly inside an executor tick. Repeated calls
/// while one is queued are coalesced.
pub fn request_resume() {
    if RESUME_QUEUED.with(|queued| queued.replace(true)) {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        RESUME_QUEUED.with(|queued| queued.set(false));
        js_yield_and_continue();
    });
}

fn wasm_bindgen_schedule_resume(dur: time::Duration, resume: Box<dyn FnOnce()>) {
    let closure = Closure::once(Box::new(move || {
        resume();
        js_yield_and_continue();
    }) as Box<dyn FnOnce()>);

    // Use js_sys::global() instead of web_sys::window() to support
    // CF Workers (DedicatedWorkerGlobalScope) and browsers (Window).
    let global = js_sys::global();
    if let Some(scope) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
        scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                dur.as_millis() as i32,
            )
            .expect("set_timeout failed");
    } else if let Some(window) = global.dyn_ref::<web_sys::Window>() {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                dur.as_millis() as i32,
            )
            .expect("set_timeout failed");
    } else {
        // Fallback: CF Workers and other JS environments — call setTimeout via Reflect
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("global has no setTimeout");
        let set_timeout = set_timeout
            .dyn_ref::<js_sys::Function>()
            .expect("setTimeout is not a function");
        let _ = set_timeout.call2(
            &global,
            closure.as_ref().unchecked_ref(),
            &JsValue::from_f64(dur.as_millis() as f64),
        );
    }
    closure.forget();
}

/// JS thread yielder using wasm-bindgen + web-sys.
/// Implements `ProcessController` with `YieldSignal = WaitStatus`.
#[derive(Clone)]
pub struct JSThreadYielder {
    spin_waiter: CooperativeSpinWaiter,
}

impl JSThreadYielder {
    pub fn new() -> Self {
        Self {
            spin_waiter: CooperativeSpinWaiter::new(100_000, wasm_bindgen_schedule_resume),
        }
    }
}

impl Default for JSThreadYielder {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::valtron::ProcessController for JSThreadYielder {
    type YieldSignal = WaitStatus;

    fn yield_for(&self, dur: time::Duration) -> WaitStatus {
        self.spin_waiter.wait(dur)
    }

    fn should_stop(&self, signal: &WaitStatus) -> bool {
        matches!(signal, WaitStatus::Waiting)
    }
}
