mod background;
mod builders;
mod collect_next;
mod config;
mod constants;
mod controller;
mod dependent_lift;
mod do_next;
mod future_task;
mod inline_action;
mod local;
mod on_next;
mod sleep;
#[cfg(feature = "std")]
mod state_task;
mod sync;
mod task_iters;
mod wrappers;

pub use background::*;
pub use builders::*;
pub use collect_next::*;
pub use config::*;
pub use constants::*;
pub use controller::*;
pub use dependent_lift::*;
pub use do_next::*;
pub use future_task::*;
pub use inline_action::*;
pub use local::*;
pub use on_next::*;
pub use sleep::*;
#[cfg(feature = "std")]
pub use state_task::*;
pub use sync::*;
pub use task_iters::*;
pub use wrappers::*;

pub mod single;

#[cfg(not(feature = "multi"))]
pub mod non_sendables;

#[cfg(not(feature = "multi"))]
pub use non_sendables::*;

#[cfg(feature = "multi")]
pub mod sendables;

#[cfg(feature = "multi")]
pub use sendables::*;

#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
pub mod wasm;

#[cfg(all(
    target_family = "wasm",
    feature = "js-wasmbindgen",
    not(feature = "js-foundation-wasm")
))]
pub use wasm::js_stream;

#[cfg(feature = "multi")]
pub mod multi;

/// Ask the executor's host to run it again because a readiness signal fired.
///
/// WHY: a task parked on `TaskStatus::Depends` becomes runnable only when the
/// signal flips. On a JS wasm build nothing re-checks the signal on its own (the
/// executor hands control back to the JS event loop rather than spin or poll on a
/// timer), so whoever flips the signal must ask for a run. [`WakeSignal`]'s
/// callback does this for you.
///
/// On JS wasm builds this queues one coalesced executor run on the event loop
/// (see the active backend's `request_resume`). Everywhere else it is a no-op:
/// native executors re-check parked tasks' readiness on their own loop.
///
/// [`WakeSignal`]: crate::valtron::WakeSignal
#[inline]
pub fn request_executor_resume() {
    #[cfg(all(
        target_family = "wasm",
        any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
    ))]
    wasm::request_resume();
}

// Re-export PoolGuard at executor level so tests can use valtron::PoolGuard
#[cfg(not(feature = "multi"))]
pub use single::PoolGuard;

#[cfg(feature = "multi")]
pub use multi::PoolGuard;

// SingleExecutorSingleton: optional convenience API for wasm targets.
// Only available when multi feature is off AND compiling for wasm32/wasm64.
#[cfg(all(not(feature = "multi"), target_family = "wasm"))]
pub use single::SingleExecutorSingleton;

// re-exported external libraries
pub use foundation_compact::rng::SeedableRng;
