//! Core backends — shared between native and wasm.

pub mod d1r2_document_store;
pub mod memory;
pub mod memory_document_store;
pub mod memory_json;
pub mod sql_document_store;

// Re-exports
pub use d1r2_document_store::D1R2DocumentStore;
pub use memory::*;
pub use memory_document_store::MemoryDocumentStore;
pub use memory_json::*;
pub use sql_document_store::SqlDocumentStore;

use foundation_compact::SystemTime;

use crate::core::errors::{StorageError, StorageResult};

/// Seconds since the Unix epoch, for the in-memory rate limiters.
///
/// Uses `foundation_compact::SystemTime`, which reads the JS clock on
/// wasm32-unknown-unknown, where `std::time::SystemTime::now` panics. A clock
/// before the epoch is reported as an error rather than a panic.
pub(crate) fn unix_now_secs() -> StorageResult<u64> {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|e| StorageError::Backend(format!("system clock is before the Unix epoch: {e:?}")))
}

/// Milliseconds since the Unix epoch, for the auth tables whose timestamp
/// columns use milliseconds (see migrations 002–027).
///
/// Uses `foundation_compact::SystemTime`, which reads the JS clock on
/// wasm32-unknown-unknown, where `std::time::SystemTime::now` panics. A clock
/// before the epoch is reported as an error rather than a panic.
pub(crate) fn unix_now_millis() -> StorageResult<u64> {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .map_err(|e| StorageError::Backend(format!("system clock is before the Unix epoch: {e:?}")))
}
