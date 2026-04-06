//! Remote-first Turso state store via libsql.
//!
//! WHY: Teams and CI/CD systems need shared state across machines.
//! Turso provides a remote SQLite database with optional embedded replicas.
//!
//! WHAT: `TursoStateStore` connects to a remote Turso database via libsql's
//! `new_remote` or `new_remote_replica` builder.
//!
//! HOW: Uses `libsql::Builder::new_remote` (remote-only) or
//! `libsql::Builder::new_remote_replica` (with local cache). Same schema
//! and helpers as `SqliteStateStore`.

use std::path::Path;
use std::sync::Arc;

use foundation_core::valtron::run_future_iter;

use super::sqlite::{
    parse_resource_row, state_to_params, to_state_stream, CREATE_TABLE_SQL, UPSERT_SQL,
};
use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageError;
use crate::rows_stream::LibsqlRowsIterator;

/// Remote-first Turso state store via libsql.
///
/// All trait methods use `schedule_future`. Connection in `Arc<libsql::Connection>`.
/// Same schema as `SqliteStateStore` — only the connection setup differs.
pub struct TursoStateStore {
    conn: Arc<libsql::Connection>,
    db: Arc<libsql::Database>,
}

impl TursoStateStore {
    /// Remote-only mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the remote connection fails.
    pub fn remote(turso_url: &str, auth_token: &str) -> Result<Self, StorageError> {
        let url = turso_url.to_string();
        let token = auth_token.to_string();
        let db = exec_future(async move { libsql::Builder::new_remote(url, token).build().await })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(format!("Turso connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: Arc::new(db),
        })
    }

    /// Embedded replica mode: local SQLite file syncs with Turso.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or replica setup fails.
    pub fn embedded_replica(
        local_path: &Path,
        turso_url: &str,
        auth_token: &str,
    ) -> Result<Self, StorageError> {
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = local_path.to_string_lossy().to_string();
        let url = turso_url.to_string();
        let token = auth_token.to_string();
        let db = exec_future(async move {
            libsql::Builder::new_remote_replica(path_str, url, token)
                .build()
                .await
        })?;
        let conn = db.connect().map_err(|e| {
            StorageError::Connection(format!("Turso replica connection failed: {e}"))
        })?;
        Ok(Self {
            conn: Arc::new(conn),
            db: Arc::new(db),
        })
    }

    /// Create from environment variables.
    ///
    /// - `TURSO_DATABASE_URL` (required)
    /// - `TURSO_AUTH_TOKEN` (required)
    /// - `TURSO_LOCAL_REPLICA` (optional path for embedded replica)
    ///
    /// # Errors
    ///
    /// Returns an error if required env vars are missing or connection fails.
    pub fn from_env() -> Result<Self, StorageError> {
        let url = std::env::var("TURSO_DATABASE_URL")
            .map_err(|_| StorageError::Connection("TURSO_DATABASE_URL must be set".to_string()))?;
        let token = std::env::var("TURSO_AUTH_TOKEN")
            .map_err(|_| StorageError::Connection("TURSO_AUTH_TOKEN must be set".to_string()))?;
        match std::env::var("TURSO_LOCAL_REPLICA") {
            Ok(path) => Self::embedded_replica(Path::new(&path), &url, &token),
            Err(_) => Self::remote(&url, &token),
        }
    }
}

impl StateStore for TursoStateStore {
    fn init(&self) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(CREATE_TABLE_SQL).await })?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                let mut stmt = conn
                    .prepare("SELECT id FROM deployment_resources ORDER BY id")
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let rows = stmt
                    .query([libsql::Value::Null; 0])
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, |row| {
                    row.get::<String>(0)
                        .map_err(|e| StorageError::SqlConversion(e.to_string()))
                }))
            },
            None,
            None,
        )
        .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM deployment_resources")
                .await?;
            let mut rows = stmt.query([libsql::Value::Null; 0]).await?;
            match rows.next().await? {
                Some(row) => {
                    let count: i64 = row.get(0)?;
                    Ok::<_, libsql::Error>(usize::try_from(count).unwrap_or(0))
                }
                None => Ok::<_, libsql::Error>(0),
            }
        })?;
        Ok(to_state_stream(stream))
    }

    fn get(
        &self,
        resource_id: &str,
    ) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let id = resource_id.to_string();
        let conn = Arc::clone(&self.conn);
        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM deployment_resources WHERE id = ?")
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt
                .query([id])
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            match rows
                .next()
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?
            {
                Some(row) => {
                    let state = parse_resource_row(&row)?;
                    Ok::<_, StorageError>(Some(state))
                }
                None => Ok(None),
            }
        })?;
        Ok(to_state_stream(stream))
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        if ids.is_empty() {
            return Ok(Box::new(std::iter::empty()));
        }
        let owned_ids: Vec<String> = ids.iter().map(|s| (*s).to_string()).collect();
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                let placeholders = owned_ids
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM deployment_resources WHERE id IN ({})",
                    placeholders
                );
                let params: Vec<libsql::Value> = owned_ids
                    .iter()
                    .map(|id| libsql::Value::Text(id.clone()))
                    .collect();

                let mut stmt = conn
                    .prepare(&sql)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let rows = stmt
                    .query(params)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_resource_row))
            },
            None,
            None,
        )
        .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                let mut stmt = conn
                    .prepare("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM deployment_resources ORDER BY id")
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let rows = stmt
                    .query([libsql::Value::Null; 0])
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_resource_row))
            },
            None,
            None,
        )
        .map_err(|e| StorageError::Backend(e.to_string()))?;

        Ok(Box::new(iter))
    }

    fn set(
        &self,
        _resource_id: &str,
        state: &ResourceState,
    ) -> Result<StateStoreStream<()>, StorageError> {
        let params = state_to_params(state)?;
        let conn = Arc::clone(&self.conn);
        let stream = schedule_future(async move {
            conn.execute(UPSERT_SQL, params)
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(())
        })?;
        Ok(to_state_stream(stream))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let id = resource_id.to_string();
        let conn = Arc::clone(&self.conn);
        let stream = schedule_future(async move {
            conn.execute("DELETE FROM deployment_resources WHERE id = ?", [id])
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(())
        })?;
        Ok(to_state_stream(stream))
    }

    fn sync_remote(&self) -> Result<(), StorageError> {
        let db = Arc::clone(&self.db);
        exec_future(async move {
            db.sync()
                .await
                .map(|_| ())
                .map_err(|e| StorageError::Backend(format!("Turso sync failed: {e}")))
        })?;
        Ok(())
    }
}
