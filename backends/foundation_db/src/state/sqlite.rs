//! Local-only SQLite state store via libsql.
//!
//! WHY: Faster than JSON files for large numbers of resources, supports
//! concurrent reads via WAL mode. No external dependencies.
//!
//! WHAT: `SqliteStateStore` wraps a local libsql database with the
//! deployment state schema. All trait methods use `schedule_future` for
//! single-value ops.
//!
//! HOW: Connection in `Arc<libsql::Connection>`, cloned into each async
//! block. `exec_future` for bootstrap (`init`), `schedule_future` for
//! trait methods. `!Send` row iterators consumed inside async blocks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_core::valtron::ThreadedValue;

use super::traits::{StateStore, StateStoreStream};
use super::types::{ResourceState, StateStatus};
use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageError;

/// SQL schema for the resources table.
pub const CREATE_TABLE_SQL: &str = r"
    PRAGMA journal_mode=WAL;
    CREATE TABLE IF NOT EXISTS deployment_resources (
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
    );
";

pub const UPSERT_SQL: &str = r"
    INSERT INTO deployment_resources (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at)
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
";

/// Plain local SQLite state store using libsql.
///
/// Local-only, no remote sync. Default path: `.deployment/state.db`.
/// Uses WAL mode for concurrent read access.
pub struct SqliteStateStore {
    conn: Arc<libsql::Connection>,
}

impl SqliteStateStore {
    /// Create a new store at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = db_path.to_string_lossy().to_string();
        let db = exec_future(async move { libsql::Builder::new_local(&path_str).build().await })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(format!("SQLite connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
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
    pub fn from_env(project_dir: &Path) -> Result<Self, StorageError> {
        let db_path = std::env::var("DEPLOYMENT_STATE_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/state.db"));
        Self::new(&db_path)
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
    use foundation_core::valtron::Stream;
    Box::new(stream.filter_map(|item| match item {
        Stream::Next(result) => Some(ThreadedValue::Value(result)),
        _ => None,
    }))
}

impl StateStore for SqliteStateStore {
    fn init(&self) -> Result<(), StorageError> {
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(CREATE_TABLE_SQL).await })?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT id FROM deployment_resources ORDER BY id")
                .await?;
            let mut rows = stmt.query([libsql::Value::Null; 0]).await?;
            let mut ids = Vec::new();
            while let Some(row) = rows.next().await? {
                let id: String = row.get(0)?;
                ids.push(id);
            }
            Ok::<_, libsql::Error>(ids)
        })?;

        use foundation_core::valtron::{ShortCircuit, Stream, StreamIteratorExt};
        let circuit = stream.map_circuit(|item| match item {
            Stream::Next(Ok(ids)) => ShortCircuit::Continue(Stream::Next(Ok(ids))),
            Stream::Next(Err(e)) => {
                ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string()))))
            }
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        let flat = circuit.flat_map_next(|result| match result {
            Ok(ids) => ids.into_iter().map(Ok).collect(),
            Err(e) => vec![Err(e)],
        });

        Ok(Box::new(flat.filter_map(|item| {
            use foundation_core::valtron::Stream;
            match item {
                Stream::Next(result) => Some(ThreadedValue::Value(result)),
                _ => None,
            }
        })))
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
        let stream = schedule_future(async move {
            let mut results = Vec::new();
            for id in &owned_ids {
                let mut stmt = conn
                    .prepare("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM deployment_resources WHERE id = ?")
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let mut rows = stmt
                    .query([id.clone()])
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                if let Some(row) = rows
                    .next()
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?
                {
                    results.push(parse_resource_row(&row)?);
                }
            }
            Ok::<_, StorageError>(results)
        })?;

        use foundation_core::valtron::{ShortCircuit, Stream, StreamIteratorExt};
        let circuit = stream.map_circuit(|item| match item {
            Stream::Next(Ok(results)) => ShortCircuit::Continue(Stream::Next(Ok(results))),
            Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        let flat = circuit.flat_map_next(|result| match result {
            Ok(states) => states.into_iter().map(Ok).collect(),
            Err(e) => vec![Err(e)],
        });

        Ok(Box::new(flat.filter_map(|item| {
            use foundation_core::valtron::Stream;
            match item {
                Stream::Next(result) => Some(ThreadedValue::Value(result)),
                _ => None,
            }
        })))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM deployment_resources ORDER BY id")
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt
                .query([libsql::Value::Null; 0])
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut results = Vec::new();
            while let Some(row) = rows
                .next()
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?
            {
                results.push(parse_resource_row(&row)?);
            }
            Ok::<_, StorageError>(results)
        })?;

        use foundation_core::valtron::{ShortCircuit, Stream, StreamIteratorExt};
        let circuit = stream.map_circuit(|item| match item {
            Stream::Next(Ok(results)) => ShortCircuit::Continue(Stream::Next(Ok(results))),
            Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        let flat = circuit.flat_map_next(|result| match result {
            Ok(states) => states.into_iter().map(Ok).collect(),
            Err(e) => vec![Err(e)],
        });

        Ok(Box::new(flat.filter_map(|item| {
            use foundation_core::valtron::Stream;
            match item {
                Stream::Next(result) => Some(ThreadedValue::Value(result)),
                _ => None,
            }
        })))
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
}
