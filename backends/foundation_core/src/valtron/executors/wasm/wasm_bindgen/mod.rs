//! wasm-bindgen + web-sys backend for JS event loop yielding.
//!
//! Uses `window.setTimeout` directly for timer scheduling.

use crate::valtron::{ProgressIndicator, State};
use foundation_nostd::primitives::cooperative_spin_waiter::{
    CooperativeSpinWaiter, WaitStatus,
};
use std::time;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// JS-specific timeout for "queue empty, check again soon" scenarios.
/// 4ms: browser minimum (clamped), CF Workers fire ~1ms but 4ms is safe.
pub const JS_WAIT_CHECK_INTERVAL: time::Duration = time::Duration::from_millis(4);

/// Module-level checker used by both the executor's `run_until` and the
/// setTimeout callback's re-invocation. Returns true when the executor
/// should stop and yield back to JS.
fn js_should_stop(signal: ProgressIndicator) -> bool {
    matches!(
        signal,
        ProgressIndicator::NoWork
            | ProgressIndicator::SpinWait(_)
            | ProgressIndicator::Wait
            | ProgressIndicator::CanProgress(Some(State::Reschedule))
    )
}

/// Module-level function called from the setTimeout closure.
/// After the timer fires and the resumed flag is set, this re-invokes
/// the valtron executor to continue processing.
fn js_yield_and_continue() {
    crate::valtron::single::run_until(js_should_stop);
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
        web_sys::console::log_1(&"JSThreadYielder: using DedicatedWorkerGlobalScope".into());
        scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                dur.as_millis() as i32,
            )
            .expect("set_timeout failed");
    } else if let Some(window) = global.dyn_ref::<web_sys::Window>() {
        web_sys::console::log_1(&"JSThreadYielder: using Window".into());
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                dur.as_millis() as i32,
            )
            .expect("set_timeout failed");
    } else {
        web_sys::console::log_1(&"JSThreadYielder: no matching global scope!".into());
        panic!("no global scope found for setTimeout");
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
