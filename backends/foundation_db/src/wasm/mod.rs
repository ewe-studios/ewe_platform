//! Wasm module — optional wasm-bindgen bridge for Cloudflare Workers.

#[cfg(feature = "wasm-bindgen-bindings")]
pub mod bindgen;

#[cfg(feature = "wasm-bindgen-bindings")]
pub use bindgen::*;
