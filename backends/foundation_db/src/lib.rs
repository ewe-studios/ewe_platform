//! Foundation DB - Unified Storage Backend
//!
//! # Architecture
//!
//! - **`core/`** — shared between native and wasm (traits, errors, memory backends,
//!   D1/R2 HTTP backends, schema, crypto, cleanup)
//! - **`native/`** — non-wasm only (turso, libsql, json_file, rows_stream)
//! - **`wasm/`** — wasm32 only (CF D1/R2/KV wasm-bindgen bindings)
//!
//! # Storage Providers
//!
//! - Turso - `SQLite`-compatible embedded/remote database with sync API
//! - libsql - Local/remote `SQLite` with sync API
//! - Cloudflare D1 - Edge `SQLite` for Cloudflare Workers
//! - Cloudflare R2 - Object storage for larger blobs
//! - In-Memory - Ephemeral storage for development/testing
//! - JSON File - Simple JSON-on-disk key-value store (native only)

// Shared — always compiled
pub mod core;

// Central storage provider (wraps all backends)
pub mod storage_provider;

// Native — TCP/DB backends, rows streaming
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

// Wasm — optional wasm-bindgen bridge for Cloudflare Workers (D1/R2/KV)
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-bindings"))]
pub mod wasm;

// State module (may contain native-only items — check contents)
pub mod state;

// Public re-exports (shared — always available)
pub use core::errors::*;
pub use core::storage_provider as traits;
pub use core::cleanup::*;
pub use core::backends::*;
pub use core::crypto::*;
pub use core::schema::*;

// Central StorageProvider + StorageBackend (always available)
pub use storage_provider::{StorageBackend, StorageProvider};

// Re-export trait types at top level for convenience
pub use core::storage_provider::{
    AsyncQueryStore, BlobStore, DataValue, FromDataValue, KeyValueStore, QueryStore,
    RateLimiterStore, SqlRow, StorageItemStream,
};

// Native-only re-exports
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

// Wasm-bindgen storage re-exports (wasm32 only)
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
pub use wasm::{D1WasmStorage, KVWasmStorage, R2WasmStorage, WasmCredentialStore, init_valtron};

// Wasm-bindgen CF types re-exports (wasm32 only)
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
pub use wasm::bindgen::{D1Database, D1PreparedStatement, KVNamespace, R2Bucket, R2Object, R2Objects};

// State re-exports
pub use state::*;
