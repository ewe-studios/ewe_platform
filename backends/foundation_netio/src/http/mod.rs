//! HTTP client module — the new home for the unified HTTP client (F51 Stage 4).
//!
//! This module re-exports the native client types at their new canonical path.
//! The old `simple_http::client` paths continue to work through the migration
//! window; callers should migrate to these paths gradually.

// Re-export the native client at the new canonical location.
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use crate::simple_http::client::native::*;
