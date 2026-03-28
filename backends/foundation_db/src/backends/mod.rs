//! Storage backend implementations.

pub mod async_utils;
pub mod memory;
pub mod json_file;
#[cfg(feature = "turso")]
pub mod turso_backend;
#[cfg(feature = "libsql")]
pub mod libsql_backend;

// Re-export main backend types for convenience
pub use memory::MemoryStorage;
pub use json_file::JsonFileStorage;
#[cfg(feature = "turso")]
pub use turso_backend::TursoStorage;
#[cfg(feature = "libsql")]
pub use libsql_backend::LibsqlStorage;
