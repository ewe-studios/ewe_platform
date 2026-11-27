//! Wasm-bindgen storage trait implementations for Cloudflare Workers.

#[cfg(feature = "wasm-bindgen-storage")]
pub mod d1_wasm;

/// A D1 executor for the Durable Streams SQL virtual file system (VFS).
#[cfg(feature = "wasm-bindgen-storage")]
pub mod d1_executor;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod r2_wasm;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod kv_wasm;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod d1r2_document_store;

#[cfg(feature = "wasm-bindgen-storage")]
pub use d1_wasm::D1WasmStorage;

/// A D1 executor for the Durable Streams SQL VFS.
#[cfg(feature = "wasm-bindgen-storage")]
pub use d1_executor::D1Executor;

#[cfg(feature = "wasm-bindgen-storage")]
pub use r2_wasm::R2WasmStorage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use kv_wasm::KVWasmStorage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use d1r2_document_store::CfD1R2DocumentStore;
