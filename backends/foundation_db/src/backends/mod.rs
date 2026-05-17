//! Storage backend implementations.

pub mod async_utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod d1_kvstore;
pub mod json_file;
#[cfg(feature = "libsql")]
pub mod libsql_backend;
pub mod memory;
#[cfg(not(target_arch = "wasm32"))]
pub mod r2_blobstore;
#[cfg(feature = "turso")]
pub mod turso_backend;

// Re-export main backend types for convenience
#[cfg(not(target_arch = "wasm32"))]
pub use d1_kvstore::D1KeyValueStore;
pub use json_file::JsonFileStorage;
#[cfg(feature = "libsql")]
pub use libsql_backend::LibsqlStorage;
pub use memory::MemoryStorage;
#[cfg(not(target_arch = "wasm32"))]
pub use r2_blobstore::R2BlobStore;
#[cfg(feature = "turso")]
pub use turso_backend::TursoStorage;
