//! Core backends — shared between native and wasm.

pub mod memory;
pub mod async_utils;

// HTTP-based Cloudflare backends — work on both targets.
#[cfg(feature = "d1")]
pub mod d1_kvstore;

#[cfg(feature = "r2")]
pub mod r2_blobstore;

// Re-exports
pub use memory::*;
pub use async_utils::*;

#[cfg(feature = "d1")]
pub use d1_kvstore::D1KeyValueStore;

#[cfg(feature = "r2")]
pub use r2_blobstore::R2BlobStore;
