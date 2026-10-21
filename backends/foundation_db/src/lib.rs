//! Foundation DB - Unified Storage Backend
//!
//! # Architecture
//!
//! - **`core/`** — shared between native and wasm (traits, errors, memory backends,
//!   D1/R2 HTTP backends, schema, crypto, cleanup)
//! - **`native/`** — non-wasm only (turso, libsql, `json_file`, `rows_stream`)
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

// `Blocking<S>`: the sync storage traits over the async ones. Only where
// valtron can block — native and non-JS wasm, never JS-hosted wasm (Cloudflare
// Workers, browsers), where blocking would freeze the isolate. The gate is
// foundation_core's (`valtron::BLOCKING_BOUNDARY`), evaluated with its features.
foundation_core::blocking_boundary! {
    if {
        mod blocking;
        pub use blocking::Blocking;

        /// Whether [`Blocking`] exists in this build (it does). Mirrors
        /// `foundation_core::valtron::BLOCKING_BOUNDARY`.
        pub const HAS_BLOCKING: bool = true;
    } else {
        /// Whether `Blocking` exists in this build: not on JS-hosted wasm,
        /// where callers `.await` the `Async*` storage traits instead.
        pub const HAS_BLOCKING: bool = false;
    }
}

use foundation_core::valtron::BLOCKING_BOUNDARY;

// Compile-time gate checks. Native always has `Blocking`. The Cloudflare JS
// bindings (`wasm-bindgen-bindings`) turn on `foundation_core/js-wasmbindgen`,
// so with them on wasm `Blocking` must be gone.
#[cfg(not(target_family = "wasm"))]
const _: () = assert!(HAS_BLOCKING, "Blocking must exist on native targets");
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-bindings"))]
const _: () = assert!(
    !HAS_BLOCKING,
    "Blocking must not exist on JS-hosted wasm (it would freeze the isolate)"
);
const _: () = assert!(HAS_BLOCKING == BLOCKING_BOUNDARY);

// Native — TCP/DB backends, rows streaming
#[cfg(not(target_family = "wasm"))]
pub mod native;

// Wasm — optional wasm-bindgen bridge for Cloudflare Workers (D1/R2/KV)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-bindings"))]
pub mod wasm;

// Public re-exports (shared — always available)
pub use core::errors::*;
pub use core::storage_provider as traits;
pub use core::cleanup::*;
pub use core::backends::*;
pub use core::crypto::*;
pub use core::schema::*;
pub use core::auth_store::*;

// Central StorageProvider + StorageBackend (always available)
pub use storage_provider::{StorageBackend, StorageProvider};

// Re-export trait types at top level for convenience
pub use core::storage_provider::{
    AsyncBlobStore, AsyncDocumentStore, AsyncKeyValueStore, AsyncListStream, AsyncQueryStore,
    AsyncQueryStream, AsyncRateLimiterStore, AsyncStorageItemStream, BlobStore, DataValue,
    Document, DocumentStore, FromDataValue, KeyValueStore, PromotableDocument, QueryStore,
    RateLimiterStore, SqlRow, StorageItemStream,
};

// Native-only re-exports
#[cfg(not(target_family = "wasm"))]
pub use native::*;

// Wasm-bindgen storage re-exports (wasm32 only)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
pub use wasm::{CfD1R2DocumentStore, D1WasmStorage, KVWasmStorage, R2WasmStorage, WasmCredentialStore};

// Wasm-bindgen CF types re-exports (wasm32 only)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
pub use wasm::bindgen::{D1Database, D1PreparedStatement, KVNamespace, R2Bucket, R2Object, R2Objects};

// State store module
pub use core::state;
