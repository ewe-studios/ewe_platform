//! Browser and Cloudflare storage bindings.

#[cfg(feature = "browser-storage")]
pub mod browser;

#[cfg(feature = "wasm-bindgen-bindings")]
pub mod bindgen;

#[cfg(feature = "wasm-bindgen-bindings")]
pub use bindgen::*;

#[cfg(any(feature = "wasm-bindgen-storage", feature = "do-sql"))]
pub mod wasm_storage;

#[cfg(feature = "wasm-bindgen-storage")]
pub use wasm_storage::*;

/// A Durable Object (DO) SQLite executor for the Durable Streams SQL VFS.
#[cfg(feature = "do-sql")]
pub use wasm_storage::DoSqlExecutor;

/// workers-rs interop: `From<worker::D1Database>` for `D1Database`.
#[cfg(feature = "workers-rs")]
pub mod workers_rs;
