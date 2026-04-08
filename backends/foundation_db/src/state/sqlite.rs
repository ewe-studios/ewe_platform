//! Local-only SQLite state store via libsql.
//!
//! WHY: Faster than JSON files for large numbers of resources, supports
//! concurrent reads via WAL mode. No external dependencies.
//!
//! WHAT: `SqliteStateStore` wraps a local libsql database with the
//! deployment state schema. Table names use `{project}_{stage}_resources`
//! pattern for namespacing. All trait methods use `schedule_future` for
//! single-value ops.
//!
//! HOW: Connection in `Arc<libsql::Connection>`, cloned into each async
//! block. `exec_future` for bootstrap (`init`), `schedule_future` for
//! trait methods. `!Send` row iterators consumed inside async blocks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_core::valtron::{run_future_iter, Stream, ThreadedValue};

use super::traits::{StateStore, StateStoreStream};
use super::types::{ResourceState, StateStatus};
use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageError;
use crate::rows_stream::LibsqlRowsIterator;

/// SQL schema for the resources table (table name is parameterized).
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

/// Plain local SQLite state store using libsql.
///
/// Local-only, no remote sync. Default path: `{project_dir}/.deployment/state.db`.
/// Uses WAL mode for concurrent read access.
///
/// Table names are prefixed with `{project}_{stage}_` for namespacing.
pub struct SqliteStateStore {
    conn: Arc<libsql::Connection>,
    table_name: String,  // "{project}_{stage}_resources"
}

impl SqliteStateStore {
    /// Create a new store at the given path.
    ///
    /// Table name will be `{project}_{stage}_resources` for namespacing.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub fn new(db_path: &Path, project: &str, stage: &str) -> Result<Self, StorageError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = db_path.to_string_lossy().to_string();
        let table_name = format!("{}_{}_resources",
            project.replace('-', "_").replace(' ', "_"),
            stage.replace('-', "_").replace(' ', "_")
        );
        let db = exec_future(async move { libsql::Builder::new_local(&path_str).build().await })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(format!("SQLite connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            table_name,
        })
    }

    /// Create from environment or default path.
    ///
    /// Checks `DEPLOYMENT_STATE_DB` env var, falls back to
    /// `{project_dir}/.deployment/state.db`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub fn from_env(project_dir: &Path, project: &str, stage: &str) -> Result<Self, StorageError> {
        let db_path = std::env::var("DEPLOYMENT_STATE_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/state.db"));
        Self::new(&db_path, project, stage)
    }
}

/// Parse a row into a `ResourceState`.
///
/// Column order must match the SELECT in queries below:
/// id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at
pub fn parse_resource_row(row: &libsql::Row) -> Result<ResourceState, StorageError> {
    let id: String = row
        .get(0)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let kind: String = row
        .get(1)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let provider: String = row
        .get(2)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let status_str: String = row
        .get(3)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let environment: Option<String> = row
        .get(4)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let config_hash: String = row
        .get(5)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let output_str: String = row
        .get(6)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let snapshot_str: String = row
        .get(7)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let created_str: String = row
        .get(8)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
    let updated_str: String = row
        .get(9)
        .map_err(|e| StorageError::SqlConversion(e.to_string()))?;

    let status: StateStatus = serde_json::from_str(&status_str)
        .map_err(|e| StorageError::Serialization(format!("status parse: {e}")))?;
    let output: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| StorageError::Serialization(format!("output parse: {e}")))?;
    let config_snapshot: serde_json::Value = serde_json::from_str(&snapshot_str)
        .map_err(|e| StorageError::Serialization(format!("config_snapshot parse: {e}")))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| StorageError::Serialization(format!("created_at parse: {e}")))?;
    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| StorageError::Serialization(format!("updated_at parse: {e}")))?;

    Ok(ResourceState {
        id,
        kind,
        provider,
        status,
        environment,
        config_hash,
        output,
        config_snapshot,
        created_at,
        updated_at,
    })
}

/// Serialize a `ResourceState` into SQL parameter values.
pub fn state_to_params(state: &ResourceState) -> Result<Vec<libsql::Value>, StorageError> {
    let status_json = serde_json::to_string(&state.status)
        .map_err(|e| StorageError::Serialization(e.to_string()))?;
    let output_json = serde_json::to_string(&state.output)
        .map_err(|e| StorageError::Serialization(e.to_string()))?;
    let snapshot_json = serde_json::to_string(&state.config_snapshot)
        .map_err(|e| StorageError::Serialization(e.to_string()))?;

    Ok(vec![
        libsql::Value::Text(state.id.clone()),
        libsql::Value::Text(state.kind.clone()),
        libsql::Value::Text(state.provider.clone()),
        libsql::Value::Text(status_json),
        match &state.environment {
            Some(env) => libsql::Value::Text(env.clone()),
            None => libsql::Value::Null,
        },
        libsql::Value::Text(state.config_hash.clone()),
        libsql::Value::Text(output_json),
        libsql::Value::Text(snapshot_json),
        libsql::Value::Text(state.created_at.to_rfc3339()),
        libsql::Value::Text(state.updated_at.to_rfc3339()),
    ])
}

/// Convert a `schedule_future` stream into a `StateStoreStream`.
///
/// Maps the `Stream`-based iterator from `schedule_future` into
/// `ThreadedValue`-based items expected by `StateStoreStream`.
pub fn to_state_stream<T: Send + 'static>(
    stream: impl foundation_core::valtron::StreamIterator<D = Result<T, StorageError>, P = ()>
        + Send
        + 'static,
) -> StateStoreStream<T> {
    Box::new(stream.filter_map(|item| match item {
        Stream::Next(result) => Some(ThreadedValue::Value(result)),
        _ => None,
    }))
}

impl StateStore for SqliteStateStore {
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
}
