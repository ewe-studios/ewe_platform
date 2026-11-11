//! JS event loop yield backends.
//!
//! Selects and exposes the correct `JSThreadYielder` implementation based on
//! feature flags. The public API is always `JSThreadYielder` regardless of
//! which feature is active.
//!
//! Priority: `js-foundation-wasm` > `js-wasmbindgen` (when both are enabled,
//! foundation-wasm wins).

// These JS event-loop yield backends are inherently single-threaded-wasm: they call
// host FFI (`register_schedule`/`requestAnimationFrame`-style yielding) and use
// closures that aren't `Send`/`Sync`. They are gated to wasm targets so a native
// build with one of these features doesn't try to compile wasm-only code (the
// native default is `NoThreadController` — see `single/mod.rs`).
#[cfg(all(
    target_family = "wasm",
    feature = "js-wasmbindgen",
    not(feature = "js-foundation-wasm")
))]
mod wasm_bindgen;
#[cfg(all(
    target_family = "wasm",
    feature = "js-wasmbindgen",
    not(feature = "js-foundation-wasm")
))]
pub use wasm_bindgen::js_stream;
#[cfg(all(
    target_family = "wasm",
    feature = "js-wasmbindgen",
    not(feature = "js-foundation-wasm")
))]
pub use wasm_bindgen::{request_resume, JSThreadYielder};

#[cfg(all(target_family = "wasm", feature = "js-foundation-wasm"))]
mod foundation_wasm;
#[cfg(all(target_family = "wasm", feature = "js-foundation-wasm"))]
pub use foundation_wasm::{request_resume, JSThreadYielder};

// Re-export WaitStatus from foundation_nostd for executor use
pub use foundation_nostd::primitives::cooperative_spin_waiter::WaitStatus;
