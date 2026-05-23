//! Wasm module — optional wasm-bindgen bridge for Cloudflare Workers.

#[cfg(feature = "wasm-bindgen-bindings")]
pub mod bindgen;

#[cfg(feature = "wasm-bindgen-bindings")]
pub use bindgen::*;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod wasm_storage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use wasm_storage::*;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod wasm_credential_store;

#[cfg(feature = "wasm-bindgen-storage")]
pub use wasm_credential_store::WasmCredentialStore;

/// Not activated for review only.
// #[cfg(feature = "wasm-bindgen-storage")]
// pub mod session;

/// workers-rs interop: `From<worker::D1Database>` for `D1Database`.
#[cfg(feature = "workers-rs")]
pub mod workers_rs;
