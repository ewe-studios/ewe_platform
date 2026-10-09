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

use foundation_db::core::storage_provider::AsyncQueryStore;

use crate::core::error::{AppError, AppResult};

pub mod ciphers;
pub mod devices;
pub mod events;
pub mod folders;
pub mod orgs;
pub mod sends;
pub mod two_factor;
pub mod users;

/// Initial schema DDL (single source of truth, shared by both backends + tests).
pub const SCHEMA: &str = include_str!("../../../migrations/0001_initial.sql");

/// Apply [`SCHEMA`] one statement at a time.
///
/// WHY: `execute_batch` is not reliably multi-statement across every
/// `foundation_db` SQL backend, so we strip comment lines, split on `;`, and run
/// each non-empty statement via `execute_async` (which every backend supports).
/// DDL is idempotent (`CREATE TABLE IF NOT EXISTS`), so this doubles as the
/// migration runner for the server backends.
pub async fn apply_schema(db: &dyn AsyncQueryStore) -> AppResult<()> {
    // Strip `--` comments (full-line and inline) before splitting; the DDL has no
    // string literals containing `--`, so a plain cut at the first `--` is safe.
    let sql: String = SCHEMA
        .lines()
        .map(|line| match line.find("--") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    for statement in sql.split(';') {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        db.execute_async(statement, &[]).await.map_err(store_err)?;
    }
    Ok(())
}

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
