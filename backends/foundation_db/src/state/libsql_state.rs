//! Embedded libsql state store with optional Turso sync.
//!
//! WHY: Combines the speed of local SQLite with optional remote sync to Turso
//! for cross-machine state sharing.
//!
//! WHAT: `LibSQLStateStore` wraps an embedded libsql database that can
//! optionally replicate to a remote Turso instance.
//!
//! HOW: Same schema and queries as `SqliteStateStore`. Only the connection
//! setup differs — `Builder::new_local` for local-only,
//! `Builder::new_remote_replica` for sync mode.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_core::valtron::ThreadedValue;

use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageError;
use super::sqlite::{CREATE_TABLE_SQL, UPSERT_SQL, parse_resource_row, state_to_params, to_state_stream};
use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;

/// Embedded libsql state store with optional Turso remote sync.
///
/// Modes:
///   1. Local-only: behaves like `SqliteStateStore` (no sync)
///   2. Embedded replica: local file with automatic background sync to Turso
pub struct LibSQLStateStore {
    conn: Arc<libsql::Connection>,
    db: Arc<libsql::Database>,
    has_remote: bool,
}

impl LibSQLStateStore {
    /// Local-only mode: no remote sync.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub fn local(local_path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = local_path.to_string_lossy().to_string();
        let db = exec_future(async move {
            libsql::Builder::new_local(&path_str).build().await
        })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(format!("libsql connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: Arc::new(db),
            has_remote: false,
        })
    }

    /// Embedded replica mode: local file with automatic sync to Turso.
    ///
    /// # Errors
    ///
    /// Returns an error if the database or remote connection fails.
    pub fn with_sync(
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
    pub fn from_env(project_dir: &Path) -> Result<Self, StorageError> {
        let local_path = std::env::var("LIBSQL_LOCAL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| project_dir.join(".deployment/libsql.db"));

        match (
            std::env::var("LIBSQL_TURSO_URL"),
            std::env::var("LIBSQL_TURSO_TOKEN"),
        ) {
            (Ok(url), Ok(token)) => Self::with_sync(&local_path, &url, &token),
            (Ok(_), Err(_)) => Err(StorageError::Connection(
                "LIBSQL_TURSO_TOKEN must be set when LIBSQL_TURSO_URL is set".to_string(),
            )),
            _ => Self::local(&local_path),
        }
    }
}

impl StateStore for LibSQLStateStore {
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
            Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                StorageError::Backend(e.to_string()),
            ))),
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
            match rows.next().await.map_err(|e| StorageError::Backend(e.to_string()))? {
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
                if let Some(row) = rows.next().await.map_err(|e| StorageError::Backend(e.to_string()))? {
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
            while let Some(row) = rows.next().await.map_err(|e| StorageError::Backend(e.to_string()))? {
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

    fn sync_remote(&self) -> Result<(), StorageError> {
        if self.has_remote {
            let db = Arc::clone(&self.db);
            exec_future(async move {
                db.sync().await.map_err(|e| StorageError::Backend(format!("sync failed: {e}")))
            })?;
        }
        Ok(())
    }
}
