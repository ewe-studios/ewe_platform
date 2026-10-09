//! Wasm32 stub backends.
//!
//! Input returns `Unsupported` (no interactive stdin in browsers).
//! Display logs to `console.log` via `web-sys`. Enabled by the `wasm`
//! feature flag.

#[cfg(feature = "wasm")]
pub mod display;
#[cfg(feature = "wasm")]
pub mod input;
#[cfg(feature = "ratzilla")]
pub mod ratzilla;
