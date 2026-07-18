//! Portable persistence layer (spec-57, F008 Stage 1).
//!
//! WHY: Keep SQL and row-mapping out of the API handlers so the handlers read as
//! business logic and the same queries run on every `foundation_db` SQL backend
//! (Turso native, D1 wasm).
//!
//! WHAT: One submodule per vault entity, each exposing `async` functions over
//! `&dyn AsyncQueryStore` plus a row struct. No custom storage traits — plain
//! `execute_async`/`query_async` with `DataValue` params and `SqlRow` parsing,
//! mirroring `foundation_auth::server::services`.
//!
//! HOW: `SCHEMA` is the initial DDL, applied via `execute_batch_async` by the
//! server bootstrap (and by tests). Grown as entities are added.

use chrono::{DateTime, Utc};

use crate::core::error::AppError;

pub mod folders;

/// Initial schema DDL (single source of truth, shared by both backends + tests).
pub const SCHEMA: &str = include_str!("../../../migrations/0001_initial.sql");

/// Map a `foundation_db` storage error into the keychain's internal error.
pub(crate) fn store_err(e: foundation_db::StorageError) -> AppError {
    AppError::Internal(e.to_string())
}

/// Parse an RFC 3339 timestamp column into a UTC datetime.
pub(crate) fn parse_ts(s: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| AppError::Internal(format!("invalid timestamp {s:?}: {e}")))
}
