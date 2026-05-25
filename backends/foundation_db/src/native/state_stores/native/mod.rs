use crate::core::state::traits::StateStore;
use crate::core::state::file::FileStateStore;
use std::path::Path;

/// Select a state store backend based on environment configuration.
///
/// Priority (first match wins):
///   1. D1 — if `DEPLOYMENT_D1_DATABASE_ID` is set
///   2. R2 — if `DEPLOYMENT_R2_BUCKET` is set
///   3. Turso remote — if `TURSO_DATABASE_URL` is set
///   4. libsql with sync — if `LIBSQL_TURSO_URL` is set
///   5. libsql local — if `LIBSQL_LOCAL_PATH` or `DEPLOYMENT_STATE_DB` is set
///   6. JSON files — default fallback
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
    #[cfg(feature = "d1")]
    if std::env::var("DEPLOYMENT_D1_DATABASE_ID").is_ok() {
        return Ok(Box::new(crate::native::D1Store::from_env_state(project, stage)?));
    }

    #[cfg(feature = "r2")]
    if std::env::var("DEPLOYMENT_R2_BUCKET").is_ok() {
        return Ok(Box::new(crate::native::R2Store::from_env_state(project, stage)?));
    }

    #[cfg(feature = "libsql")]
    {
        if std::env::var("TURSO_DATABASE_URL").is_ok() {
            return Ok(Box::new(crate::native::LibsqlStore::from_env_state(project_dir, project, stage)?));
        }

        if std::env::var("LIBSQL_TURSO_URL").is_ok() {
            return Ok(Box::new(crate::native::LibsqlStore::from_env_state(project_dir, project, stage)?));
        }

        if std::env::var("LIBSQL_LOCAL_PATH").is_ok()
            || std::env::var("DEPLOYMENT_STATE_DB").is_ok()
            || project_dir.join(".deployment/state.db").exists()
        {
            return Ok(Box::new(crate::native::LibsqlStore::from_env_state(project_dir, project, stage)?));
        }
    }

    Ok(Box::new(FileStateStore::new(project_dir, project, stage)))
}
