//! Wasm module — optional wasm-bindgen bridge for Cloudflare Workers.

#[cfg(feature = "wasm-bindgen-bindings")]
pub mod bindgen;

#[cfg(feature = "wasm-bindgen-bindings")]
pub use bindgen::*;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod wasm_storage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use wasm_storage::*;
