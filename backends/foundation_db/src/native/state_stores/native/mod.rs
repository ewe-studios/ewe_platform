#[cfg(feature = "d1")]
pub mod d1;

#[cfg(feature = "r2")]
pub mod r2;

#[cfg(feature = "libsql")]
pub mod libsql_state;

#[cfg(feature = "libsql")]
pub mod sqlite;

#[cfg(feature = "libsql")]
pub mod turso;

#[cfg(feature = "libsql")]
pub use self::turso::TursoStateStore;

pub use super::shared::namespaced::NamespacedStore;
pub use super::shared::traits::{StateStore, StateStoreStream};
pub use super::shared::types::{ResourceState, StateStatus};

pub use hash::config_hash;

#[cfg(not(target_arch = "wasm32"))]
pub use d1::D1StateStore;
pub use file::FileStateStore;
pub use helpers::{collect_all, collect_first, drive_to_completion};
#[cfg(feature = "libsql")]
pub use libsql_state::LibSQLStateStore;

#[cfg(not(target_arch = "wasm32"))]
pub use r2::R2StateStore;
#[cfg(feature = "libsql")]
pub use sqlite::SqliteStateStore;
pub use store_state_task::{
    ProviderError, StoreStateIdentifierTask, StoreStatePending, StoreStateTask,
};

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
/// All stores are namespaced by project and stage to prevent state collisions
/// between different projects or stages sharing the same backend infrastructure.
///
/// # Errors
///
/// Returns an error if the selected backend fails to initialize.
pub fn create_state_store(
    project: &str,
    project_dir: &Path,
    _provider: &str,
    stage: &str,
) -> Result<Box<dyn StateStore>, crate::core::errors::StorageError> {
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var("DEPLOYMENT_D1_DATABASE_ID").is_ok() {
        return Ok(Box::new(D1StateStore::from_env(project, stage)?));
    }

    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var("DEPLOYMENT_R2_BUCKET").is_ok() {
        return Ok(Box::new(R2StateStore::from_env(project, stage)?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("TURSO_DATABASE_URL").is_ok() {
        return Ok(Box::new(TursoStateStore::from_env(project, stage)?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("LIBSQL_TURSO_URL").is_ok() {
        return Ok(Box::new(LibSQLStateStore::from_env(
            project_dir,
            project,
            stage,
        )?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("LIBSQL_LOCAL_PATH").is_ok() {
        return Ok(Box::new(LibSQLStateStore::from_env(
            project_dir,
            project,
            stage,
        )?));
    }

    #[cfg(feature = "libsql")]
    if std::env::var("DEPLOYMENT_STATE_DB").is_ok()
        || project_dir.join(".deployment/state.db").exists()
    {
        return Ok(Box::new(SqliteStateStore::from_env(
            project_dir,
            project,
            stage,
        )?));
    }

    Ok(Box::new(FileStateStore::new(project_dir, project, stage)))
}
