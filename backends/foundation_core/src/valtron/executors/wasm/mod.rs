//! JS event loop yield backends.
//!
//! Selects and exposes the correct `JSThreadYielder` implementation based on
//! feature flags. The public API is always `JSThreadYielder` regardless of
//! which feature is active.

#[cfg(feature = "js-wasmbindgen")]
mod wasm_bindgen;
#[cfg(feature = "js-wasmbindgen")]
pub use wasm_bindgen::JSThreadYielder;
#[cfg(feature = "js-wasmbindgen")]
pub use wasm_bindgen::JS_WAIT_CHECK_INTERVAL;

#[cfg(feature = "js-foundation-wasm")]
mod foundation_wasm;
#[cfg(feature = "js-foundation-wasm")]
pub use foundation_wasm::JSThreadYielder;
#[cfg(feature = "js-foundation-wasm")]
pub use foundation_wasm::JS_WAIT_CHECK_INTERVAL;

// Re-export WaitStatus from foundation_nostd for executor use
pub use foundation_nostd::primitives::cooperative_spin_waiter::WaitStatus;
