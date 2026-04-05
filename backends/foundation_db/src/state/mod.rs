//! Deployment state management — trait, types, backends, and factory.
//!
//! WHY: The deployment engine needs persistent, backend-agnostic state to track
//! what's deployed, detect config changes, and coordinate rollbacks.
//!
//! WHAT: `StateStore` trait with six interchangeable backends (JSON files,
//! `SQLite`, libsql, Turso, Cloudflare R2, Cloudflare D1), plus helpers
//! and a factory for auto-detection.
//!
//! HOW: All backends implement the same `StateStore` trait. I/O methods return
//! `StateStoreStream<T>` (lazy iterators). The factory selects a backend
//! from environment variables.

pub mod types;
pub mod traits;
pub mod helpers;
pub mod hash;
pub mod file;
#[cfg(feature = "libsql")]
pub mod sqlite;
#[cfg(feature = "libsql")]
pub mod libsql_state;
#[cfg(feature = "libsql")]
pub mod turso;
pub mod r2;
pub mod d1;

pub use types::{ResourceState, StateStatus};
pub use traits::{StateStore, StateStoreStream};
pub use helpers::{collect_first, collect_all, drive_to_completion};
pub use hash::config_hash;
pub use file::FileStateStore;
#[cfg(feature = "libsql")]
pub use sqlite::SqliteStateStore;
#[cfg(feature = "libsql")]
pub use libsql_state::LibSQLStateStore;
#[cfg(feature = "libsql")]
pub use self::turso::TursoStateStore;
pub use r2::R2StateStore;
pub use d1::D1StateStore;

use std::path::Path;

/// Select a state store backend based on environment configuration.
///
/// Priority (first match wins):
///   1. D1 — if `DEPLOYMENT_D1_DATABASE_ID` is set
///   2. R2 — if `DEPLOYMENT_R2_BUCKET` is set
///   3. Turso — if `TURSO_DATABASE_URL` is set
///   4. libsql with sync — if `LIBSQL_TURSO_URL` is set
///   5. libsql local — if `LIBSQL_LOCAL_PATH` is set
///   6. `SQLite` (local-only) — if `DEPLOYMENT_STATE_DB` is set
///   7. JSON files — default fallback
///
/// # Errors
///
/// Returns an error if the selected backend fails to initialize.
pub fn create_state_store(
    project_dir: &Path,
    provider: &str,
    stage: &str,
) -> Result<Box<dyn StateStore>, crate::errors::StorageError> {
    if std::env::var("DEPLOYMENT_D1_DATABASE_ID").is_ok() {
        return Ok(Box::new(D1StateStore::from_env()?));
    }

    if std::env::var("DEPLOYMENT_R2_BUCKET").is_ok() {
        return Ok(Box::new(R2StateStore::from_env()?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("TURSO_DATABASE_URL").is_ok() {
        return Ok(Box::new(TursoStateStore::from_env()?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("LIBSQL_TURSO_URL").is_ok() {
        return Ok(Box::new(LibSQLStateStore::from_env(project_dir)?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("LIBSQL_LOCAL_PATH").is_ok() {
        return Ok(Box::new(LibSQLStateStore::from_env(project_dir)?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("DEPLOYMENT_STATE_DB").is_ok()
        || project_dir.join(".deployment/state.db").exists()
    {
        return Ok(Box::new(SqliteStateStore::from_env(project_dir)?));
    }

    Ok(Box::new(FileStateStore::new(project_dir, provider, stage)))
}
