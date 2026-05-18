//! Native module — non-wasm only.

pub mod storage_provider;

#[cfg(feature = "turso")]
pub mod turso_backend;

#[cfg(feature = "libsql")]
pub mod libsql_backend;

pub mod json_file;
pub mod rows_stream;

pub use storage_provider::*;

#[cfg(feature = "turso")]
pub use turso_backend::TursoStorage;

#[cfg(feature = "libsql")]
pub use libsql_backend::LibsqlStorage;

pub use json_file::JsonFileStorage;
pub use rows_stream::*;
