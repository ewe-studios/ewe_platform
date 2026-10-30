//! foundation_wasm FFI backend for JS event loop yielding.
//!
//! Uses `foundation_wasm::abi::web::register_schedule` for timer scheduling.

use crate::valtron::{ProgressIndicator, State};
use foundation_nostd::primitives::cooperative_spin_waiter::{
    CooperativeSpinWaiter, WaitStatus,
};
use std::sync::Mutex;
use std::time;

/// Module-level checker used by both the executor's `run_until` and the
/// timer callback's re-invocation. Returns true when the executor
/// should stop and yield back to JS.
///
/// NOTE: `Wait` and `SpinWait` are deliberately NOT stop states. They are
/// "yield" states: `run_until` handles them in its own arms (scheduling a
/// fresh timer before returning). If the checker stopped on them, the timer
/// re-invocation would break on the very first `Wait` without rescheduling,
/// and a task parked on in-flight async work would never be re-polled.
fn js_should_stop(signal: ProgressIndicator) -> bool {
    matches!(
        signal,
        ProgressIndicator::NoWork | ProgressIndicator::CanProgress(Some(State::Reschedule))
    )
}

/// Module-level function called from the ScheduleRegistry callback.
/// After the timer fires and the resumed flag is set, this re-invokes
/// the valtron executor to continue processing. A no-op when this thread never
/// initialized a pool (a yield outside any executor still queues the timer).
fn js_yield_and_continue() {
    if crate::valtron::single::is_initialized() {
        crate::valtron::single::run_until(js_should_stop);
    }
}

/// Set while a resume microtask is queued, so a burst of wakes queues a single
/// executor run instead of one per wake.
static RESUME_QUEUED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ask the host event loop to run the valtron executor again — the event-driven
/// counterpart to the timer yield (see the wasm-bindgen backend's
/// `request_resume`).
///
/// HOW: queues a host microtask (`abi::web::register_microtask`, JS
/// `queueMicrotask`), so the executor runs as soon as the current JS task
/// finishes and never re-entrantly inside an executor tick. Repeated calls while
/// one is queued are coalesced.
pub fn request_resume() {
    use std::sync::atomic::Ordering;
    if RESUME_QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    foundation_wasm::abi::web::register_microtask(move || {
        RESUME_QUEUED.store(false, Ordering::Release);
        js_yield_and_continue();
    });
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
