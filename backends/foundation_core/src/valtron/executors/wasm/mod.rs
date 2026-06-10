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
    any(target_arch = "wasm32", target_arch = "wasm64"),
    feature = "js-wasmbindgen",
    not(feature = "js-foundation-wasm")
))]
mod wasm_bindgen;
#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    feature = "js-wasmbindgen",
    not(feature = "js-foundation-wasm")
))]
pub use wasm_bindgen::{JSThreadYielder, JS_WAIT_CHECK_INTERVAL};

#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    feature = "js-foundation-wasm"
))]
mod foundation_wasm;
#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    feature = "js-foundation-wasm"
))]
pub use foundation_wasm::{JSThreadYielder, JS_WAIT_CHECK_INTERVAL};

// The JS yield *path* in `local.rs` is feature-gated (not target-gated): on native
// it runs against `NoThreadController` but still references `JS_WAIT_CHECK_INTERVAL`.
// The wasm-only backends above own the constant on wasm targets; provide the same
// value for native+feature builds so the shared path compiles.
#[cfg(all(
    not(any(target_arch = "wasm32", target_arch = "wasm64")),
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
pub const JS_WAIT_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(4);

// Re-export WaitStatus from foundation_nostd for executor use
pub use foundation_nostd::primitives::cooperative_spin_waiter::WaitStatus;
