//! Storage backend implementations.

pub mod async_utils;
pub mod d1_kvstore;
pub mod json_file;
#[cfg(feature = "libsql")]
pub mod libsql_backend;
pub mod memory;
pub mod r2_blobstore;
#[cfg(feature = "turso")]
pub mod turso_backend;

// Re-export main backend types for convenience
pub use d1_kvstore::D1KeyValueStore;
pub use json_file::JsonFileStorage;
#[cfg(feature = "libsql")]
pub use libsql_backend::LibsqlStorage;
pub use memory::MemoryStorage;
pub use r2_blobstore::R2BlobStore;
#[cfg(feature = "turso")]
pub use turso_backend::TursoStorage;
