//! Browser and Cloudflare storage bindings.

#[cfg(feature = "browser-storage")]
pub mod browser;

#[cfg(feature = "wasm-bindgen-bindings")]
pub mod bindgen;

#[cfg(feature = "wasm-bindgen-bindings")]
pub use bindgen::*;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod wasm_storage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use wasm_storage::*;

/// workers-rs interop: `From<worker::D1Database>` for `D1Database`.
#[cfg(feature = "workers-rs")]
pub mod workers_rs;
