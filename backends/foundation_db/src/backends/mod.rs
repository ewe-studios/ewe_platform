//! Storage backend implementations.

pub mod async_utils;
pub mod json_file;
#[cfg(feature = "libsql")]
pub mod libsql_backend;
pub mod memory;
#[cfg(feature = "turso")]
pub mod turso_backend;

// Re-export main backend types for convenience
pub use json_file::JsonFileStorage;
#[cfg(feature = "libsql")]
pub use libsql_backend::LibsqlStorage;
pub use memory::MemoryStorage;
#[cfg(feature = "turso")]
pub use turso_backend::TursoStorage;
