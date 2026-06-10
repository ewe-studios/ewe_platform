//! foundation_wasm FFI backend for JS event loop yielding.
//!
//! Uses `foundation_wasm::abi::web::register_schedule` for timer scheduling.

use crate::valtron::{ProgressIndicator, State};
use foundation_nostd::primitives::cooperative_spin_waiter::{
    CooperativeSpinWaiter, WaitStatus,
};
use std::sync::Mutex;
use std::time;

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

/// Module-level function called from the ScheduleRegistry callback.
/// After the timer fires and the resumed flag is set, this re-invokes
/// the valtron executor to continue processing.
fn js_yield_and_continue() {
    crate::valtron::single::run_until(js_should_stop);
}

fn foundation_wasm_schedule_resume(dur: time::Duration, resume: Box<dyn FnOnce()>) {
    // Wrap FnOnce in Mutex<Option> so it can satisfy Fn bound
    let resume = Mutex::new(Some(resume));
    foundation_wasm::abi::web::register_schedule(
        dur.as_millis() as f64,
        move || {
            if let Some(r) = resume.lock().unwrap().take() {
                r();
            }
            js_yield_and_continue();
        },
    );
}

/// JS thread yielder using foundation_wasm FFI.
/// Implements `ProcessController` with `YieldSignal = WaitStatus`.
#[derive(Clone)]
pub struct JSThreadYielder {
    spin_waiter: CooperativeSpinWaiter,
}

impl JSThreadYielder {
    pub fn new() -> Self {
        Self {
            spin_waiter: CooperativeSpinWaiter::new(100_000, foundation_wasm_schedule_resume),
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
