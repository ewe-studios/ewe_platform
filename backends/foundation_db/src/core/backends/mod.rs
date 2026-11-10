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

use foundation_compact::time::{unix_millis, unix_seconds};
use foundation_core::valtron::Ctx;

use crate::core::errors::{StorageError, StorageResult};

/// Seconds since the Unix epoch, read from the injected clock.
///
/// The wall time comes from `Clock::wall_now`, so a deterministic simulation
/// test controls it. A clock before the epoch is reported as an error rather
/// than a panic.
pub(crate) fn unix_now_secs(ctx: &Ctx) -> StorageResult<u64> {
    let secs = unix_seconds(ctx.clock().wall_now());
    u64::try_from(secs).map_err(|_| {
        StorageError::Backend(format!(
            "injected clock is before the Unix epoch: {secs} unix seconds"
        ))
    })
}

/// Milliseconds since the Unix epoch, read from the injected clock.
///
/// The auth tables whose timestamp columns use milliseconds (see migrations
/// 002–027) read this value. The wall time comes from `Clock::wall_now`, so a
/// deterministic simulation test controls it. A clock before the epoch is
/// reported as an error rather than a panic.
pub(crate) fn unix_now_millis(ctx: &Ctx) -> StorageResult<u64> {
    let millis = unix_millis(ctx.clock().wall_now());
    u64::try_from(millis).map_err(|_| {
        StorageError::Backend(format!(
            "injected clock is before the Unix epoch: {millis} unix milliseconds"
        ))
    })
}
