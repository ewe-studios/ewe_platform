//! Embedded libsql state store with optional Turso sync.
//!
//! WHY: Combines the speed of local SQLite with optional remote sync to Turso
//! for cross-machine state sharing.
//!
//! WHAT: `LibSQLStateStore` wraps an embedded libsql database that can
//! optionally replicate to a remote Turso instance.
//!
//! HOW: Same schema and queries as `SqliteStateStore`. Table names use
//! `{project}_{stage}_resources` pattern for namespacing. Only the connection
//! setup differs — `Builder::new_local` for local-only,
//! `Builder::new_remote_replica` for sync mode.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_core::valtron::run_future_iter;

use super::sqlite::{parse_resource_row, state_to_params, to_state_stream};
use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageError;
use crate::rows_stream::LibsqlRowsIterator;

/// Generate CREATE TABLE SQL for a given table name.
fn create_table_sql(table_name: &str) -> String {
    format!(
        r"
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS {} (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            provider TEXT NOT NULL,
            status TEXT NOT NULL,
            environment TEXT,
            config_hash TEXT NOT NULL,
            output TEXT NOT NULL,
            config_snapshot TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        ",
        table_name
    )
}

/// Generate UPSERT SQL for a given table name.
fn upsert_sql(table_name: &str) -> String {
    format!(
        r"
        INSERT INTO {} (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            kind = excluded.kind,
            provider = excluded.provider,
            status = excluded.status,
            environment = excluded.environment,
            config_hash = excluded.config_hash,
            output = excluded.output,
            config_snapshot = excluded.config_snapshot,
            updated_at = excluded.updated_at
        ",
        table_name
    )
}

/// Embedded libsql state store with optional Turso remote sync.
///
/// Modes:
///   1. Local-only: behaves like `SqliteStateStore` (no sync)
///   2. Embedded replica: local file with automatic background sync to Turso
///
/// Table names are prefixed with `{project}_{stage}_` for namespacing.
pub struct LibSQLStateStore {
    conn: Arc<libsql::Connection>,
    db: Arc<libsql::Database>,
    has_remote: bool,
    table_name: String,  // "{project}_{stage}_resources"
}

impl LibSQLStateStore {
    /// Local-only mode: no remote sync.
    ///
    /// Table name will be `{project}_{stage}_resources` for namespacing.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub fn local(local_path: &Path, project: &str, stage: &str) -> Result<Self, StorageError> {
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = local_path.to_string_lossy().to_string();
        let table_name = format!("{}_{}_resources",
            project.replace('-', "_").replace(' ', "_"),
            stage.replace('-', "_").replace(' ', "_")
        );
        let db = exec_future(async move { libsql::Builder::new_local(&path_str).build().await })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(format!("libsql connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: Arc::new(db),
            has_remote: false,
            table_name,
        })
    }

    /// Embedded replica mode: local file with automatic sync to Turso.
    ///
    /// Table name will be `{project}_{stage}_resources` for namespacing.
    ///
    /// # Errors
    ///
    /// Returns an error if the database or remote connection fails.
    pub fn with_sync(
        local_path: &Path,
        project: &str,
        stage: &str,
        turso_url: &str,
        auth_token: &str,
    ) -> Result<Self, StorageError> {
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = local_path.to_string_lossy().to_string();
        let url = turso_url.to_string();
        let token = auth_token.to_string();
        let table_name = format!("{}_{}_resources",
            project.replace('-', "_").replace(' ', "_"),
            stage.replace('-', "_").replace(' ', "_")
        );
        let db = exec_future(async move {
            libsql::Builder::new_remote_replica(&path_str, url, token)
                .build()
                .await
        })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(format!("libsql sync connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: Arc::new(db),
            has_remote: true,
            table_name,
        })
    }

    /// Create from environment variables.
    ///
    /// - `LIBSQL_LOCAL_PATH` (optional, default: `.deployment/libsql.db`)
    /// - `LIBSQL_TURSO_URL` (optional — if set, enables sync)
    /// - `LIBSQL_TURSO_TOKEN` (required if `LIBSQL_TURSO_URL` is set)
    ///
    /// # Errors
    ///
    /// Returns an error if required env vars are missing or connection fails.
    pub fn from_env(project_dir: &Path, project: &str, stage: &str) -> Result<Self, StorageError> {
        let local_path = std::env::var("LIBSQL_LOCAL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/libsql.db"));

        match (
            std::env::var("LIBSQL_TURSO_URL"),
            std::env::var("LIBSQL_TURSO_TOKEN"),
        ) {
            (Ok(url), Ok(token)) => Self::with_sync(&local_path, project, stage, &url, &token),
            (Ok(_), Err(_)) => Err(StorageError::Connection(
                "LIBSQL_TURSO_TOKEN must be set when LIBSQL_TURSO_URL is set".to_string(),
            )),
            _ => Self::local(&local_path, project, stage),
        }
    }
}

impl StateStore for LibSQLStateStore {
    fn init(&self) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        let sql = create_table_sql(&self.table_name);
        exec_future(async move { conn.execute_batch(&sql).await })?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let table = self.table_name.clone();

        let iter = run_future_iter(
            move || async move {
                let sql = format!("SELECT id FROM {} ORDER BY id", table);
                let mut stmt = conn
                    .prepare(&sql)
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
        let table = self.table_name.clone();
        let stream = schedule_future(async move {
            let sql = format!("SELECT COUNT(*) FROM {}", table);
            let mut stmt = conn
                .prepare(&sql)
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
        let table = self.table_name.clone();
        let stream = schedule_future(async move {
            let sql = format!(
                "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id = ?",
                table
            );
            let mut stmt = conn
                .prepare(&sql)
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
        let table = self.table_name.clone();

        let iter = run_future_iter(
            move || async move {
                let placeholders = owned_ids
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} WHERE id IN ({})",
                    table, placeholders
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
        let table = self.table_name.clone();

        let iter = run_future_iter(
            move || async move {
                let sql = format!(
                    "SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {} ORDER BY id",
                    table
                );
                let mut stmt = conn
                    .prepare(&sql)
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
        let sql = upsert_sql(&self.table_name);
        let stream = schedule_future(async move {
            conn.execute(&sql, params)
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(())
        })?;
        Ok(to_state_stream(stream))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let id = resource_id.to_string();
        let conn = Arc::clone(&self.conn);
        let table = self.table_name.clone();
        let stream = schedule_future(async move {
            let sql = format!("DELETE FROM {} WHERE id = ?", table);
            conn.execute(&sql, [id])
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(())
        })?;
        Ok(to_state_stream(stream))
    }

    fn sync_remote(&self) -> Result<(), StorageError> {
        if self.has_remote {
            let db = Arc::clone(&self.db);
            exec_future(async move {
                db.sync()
                    .await
                    .map_err(|e| StorageError::Backend(format!("sync failed: {e}")))
            })?;
        }
        Ok(())
    }
}
