//! Foundation DB - Unified Storage Backend
//!
//! # Architecture
//!
//! - **`core/`** — shared between native and wasm (traits, errors, memory backends,
//!   D1/R2 HTTP backends, schema, crypto, cleanup)
//! - **`native/`** — non-wasm targets, plus wasm with the WebAssembly System
//!   Interface (WASI) for `json_file` (turso, libsql, `json_file`, `rows_stream`)
//! - **`wasm/`** — wasm32 only (CF D1/R2/KV wasm-bindgen bindings)
//!
//! # Storage Providers
//!
//! - Turso - `SQLite`-compatible embedded/remote database with sync API
//! - libsql - Local/remote `SQLite` with sync API
//! - Cloudflare D1 - Edge `SQLite` for Cloudflare Workers
//! - Cloudflare R2 - Object storage for larger blobs
//! - In-Memory - Ephemeral storage for development/testing
//! - JSON File - Simple JSON-on-disk key-value store (native and WASI)

// The `turso` crate's async methods nest past the default trait-solver
// recursion limit (128). `TursoStorage::set` (an `async_trait` impl) resolves
// `Send`/`CoerceUnsized` through that chain and overflows it (issue #159228,
// `recursion_depth_exceeding_limit`). Raise the limit instead of flattening the
// chain: the depth is in the external `turso` dependency, not here.
#![recursion_limit = "256"]

// Shared — always compiled
pub mod core;

// Central storage provider (wraps all backends)
pub mod storage_provider;

// Native — TCP/DB backends, rows streaming. `json_file` also needs only
// filesystem access, which a wasm target with WASI has.
#[cfg(json_file_backend)]
pub mod native;

// Wasm — optional wasm-bindgen bridge for Cloudflare Workers (D1/R2/KV)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-bindings"))]
pub mod wasm;

// Public re-exports (shared — always available)
pub use core::auth_store::*;
pub use core::backends::*;
pub use core::cleanup::*;
pub use core::crypto::*;
pub use core::errors::*;
pub use core::schema::*;
pub use core::storage_provider as traits;

// Central StorageProvider + StorageBackend (always available)
pub use storage_provider::{BackendCtx, StorageBackend, StorageProvider};

// Re-export trait types at top level for convenience
pub use core::storage_provider::{
    AsyncListStream, AsyncQueryStream, AsyncStorageItemStream, BlobStore, DataValue, Document,
    DocumentStore, FromDataValue, KeyValueStore, PromotableDocument, QueryStore, RateLimiterStore,
    SqlRow,
};

// Native and WASI re-exports
#[cfg(json_file_backend)]
pub use native::*;

// Wasm-bindgen storage re-exports (wasm32 only)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
pub use wasm::{CfD1R2DocumentStore, D1WasmStorage, KVWasmStorage, R2WasmStorage};

// Wasm-bindgen CF types re-exports (wasm32 only)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
pub use wasm::bindgen::{
    D1Database, D1PreparedStatement, KVNamespace, R2Bucket, R2Object, R2Objects,
};

// State store module
pub use core::state;
