//! Native module — non-wasm only.
pub mod state_stores;

#[cfg(feature = "turso")]
pub mod turso_backend;

#[cfg(feature = "libsql")]
pub mod libsql_backend;

#[cfg(feature = "d1")]
pub mod d1_kvstore;

#[cfg(feature = "r2")]
pub mod r2_blobstore;

pub mod json_file;
pub mod rows_stream;

#[cfg(feature = "turso")]
pub use turso_backend::TursoStorage;

#[cfg(feature = "libsql")]
pub use libsql_backend::LibsqlStorage;

#[cfg(feature = "d1")]
pub use d1_kvstore::D1KeyValueStore;

#[cfg(feature = "r2")]
pub use r2_blobstore::R2BlobStore;

pub use json_file::JsonFileStorage;
pub use rows_stream::*;
