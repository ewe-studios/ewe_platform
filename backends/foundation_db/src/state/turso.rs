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

use foundation_core::valtron::ThreadedValue;

use super::sqlite::{
    parse_resource_row, state_to_params, to_state_stream, CREATE_TABLE_SQL, UPSERT_SQL,
};
use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageError;

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
                Some(row) => Ok::<_, StorageError>(Some(parse_resource_row(&row)?)),
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
            Stream::Next(Ok(r)) => ShortCircuit::Continue(Stream::Next(Ok(r))),
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
            Stream::Next(Ok(r)) => ShortCircuit::Continue(Stream::Next(Ok(r))),
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
