//! Wasm-bindgen storage trait implementations for Cloudflare Workers.

/// Convert between the SQL engine values and JavaScript values.
#[cfg(any(feature = "wasm-bindgen-storage", feature = "do-sql"))]
mod sql_value;

#[cfg(feature = "wasm-bindgen-storage")]
pub mod d1_wasm;

/// A D1 executor for the Durable Streams SQL virtual file system (VFS).
#[cfg(feature = "wasm-bindgen-storage")]
pub mod d1_executor;

/// A Durable Object (DO) SQLite executor for the Durable Streams SQL VFS.
#[cfg(feature = "do-sql")]
pub mod do_sql_executor;

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

/// A Durable Object (DO) SQLite executor for the Durable Streams SQL VFS.
#[cfg(feature = "do-sql")]
pub use do_sql_executor::DoSqlExecutor;

#[cfg(feature = "wasm-bindgen-storage")]
pub use r2_wasm::R2WasmStorage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use kv_wasm::KVWasmStorage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use d1r2_document_store::CfD1R2DocumentStore;
