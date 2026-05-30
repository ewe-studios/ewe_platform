//! libsql storage backend (unified).
//!
//! WHY: libsql provides embedded SQLite with optional Turso remote sync —
//! useful for KV, query execution, rate limiting, blob storage, and deployment state.
//!
//! WHAT: `LibsqlStore` implements multiple storage traits via a single
//! `libsql::Connection`, shared across all modes.
//!
//! HOW: Different constructors for different usage modes:
//!   - `LibsqlStore::new_kv()` — KV/query/rate-limit/blob store (kv_store, rate_limits, blobs tables)
//!   - `LibsqlStore::new_state()` — StateStore (table: `{project}_{stage}_resources`)
//!   - `LibsqlStore::new_state_remote()` — StateStore with Turso remote sync

use std::path::Path;
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::{
    collect_one, collect_result, from_future, run_future_iter, schedule_future, ShortCircuit, Stream, StreamIteratorExt,
    ThreadedValue,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::backends::async_utils::{exec_future, schedule_future};
use crate::core::crypto::{decrypt, encrypt, EncryptionKey};
use crate::core::errors::{StorageError, StorageResult};
use crate::core::state::traits::{StateStore, StateStoreStream};
use crate::core::state::types::{ResourceState, StateStatus};
use crate::native::rows_stream::LibsqlRowsIterator;
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncQueryStore, AsyncRateLimiterStore,
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

// ============================================================================
// State mode helpers
// ============================================================================

fn state_create_table_sql(table_name: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (id TEXT PRIMARY KEY, kind TEXT NOT NULL, provider TEXT NOT NULL, status TEXT NOT NULL, environment TEXT, config_hash TEXT NOT NULL, output TEXT NOT NULL, config_snapshot TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
    )
}

fn state_upsert_sql(table_name: &str) -> String {
    format!(
        "INSERT INTO {table_name} (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, provider = excluded.provider, status = excluded.status, environment = excluded.environment, config_hash = excluded.config_hash, output = excluded.output, config_snapshot = excluded.config_snapshot, updated_at = excluded.updated_at",
    )
}

fn state_create_indexes_sql(table_name: &str) -> String {
    format!(
        "CREATE INDEX IF NOT EXISTS idx_{table_name}_kind ON {table_name}(kind);\
         CREATE INDEX IF NOT EXISTS idx_{table_name}_provider ON {table_name}(provider);\
         CREATE INDEX IF NOT EXISTS idx_{table_name}_status ON {table_name}(status);\
         CREATE INDEX IF NOT EXISTS idx_{table_name}_environment ON {table_name}(environment);"
    )
}

fn escape_like(s: &str) -> String {
    s.replace('%', "\\%").replace('_', "\\_")
}

fn parse_state_row(row: &libsql::Row) -> Result<ResourceState, StorageError> {
    let get = |idx: usize| row.get::<String>(idx).map_err(|e| StorageError::SqlConversion(e.to_string()));
    let id = get(0)?;
    let kind = get(1)?;
    let provider = get(2)?;
    let status_str = get(3)?;
    let environment: Option<String> = row.get(4).ok();
    let config_hash = get(5)?;
    let output_str = get(6)?;
    let snapshot_str = get(7)?;
    let created_str = get(8)?;
    let updated_str = get(9)?;

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
    Ok(ResourceState { id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at })
}

fn state_to_params(state: &ResourceState) -> Result<Vec<libsql::Value>, StorageError> {
    let status_json = serde_json::to_string(&state.status).map_err(|e| StorageError::Serialization(e.to_string()))?;
    let output_json = serde_json::to_string(&state.output).map_err(|e| StorageError::Serialization(e.to_string()))?;
    let snapshot_json = serde_json::to_string(&state.config_snapshot).map_err(|e| StorageError::Serialization(e.to_string()))?;
    Ok(vec![
        libsql::Value::Text(state.id.clone()),
        libsql::Value::Text(state.kind.clone()),
        libsql::Value::Text(state.provider.clone()),
        libsql::Value::Text(status_json),
        state.environment.as_ref().map_or(libsql::Value::Null, |e| libsql::Value::Text(e.clone())),
        libsql::Value::Text(state.config_hash.clone()),
        libsql::Value::Text(output_json),
        libsql::Value::Text(snapshot_json),
        libsql::Value::Text(state.created_at.to_rfc3339()),
        libsql::Value::Text(state.updated_at.to_rfc3339()),
    ])
}

fn to_state_stream<T: Send + 'static>(
    stream: impl foundation_core::valtron::StreamIterator<D = Result<T, StorageError>, P = ()> + Send + 'static,
) -> StateStoreStream<T> {
    Box::new(stream.filter_map(|item| match item {
        Stream::Next(result) => Some(ThreadedValue::Value(result)),
        _ => None,
    }))
}

// ============================================================================
// Mode enum
// ============================================================================

#[derive(Clone)]
enum LibsqlMode {
    /// KV/query/rate-limit/blob store. Uses kv_store, rate_limits, blobs tables.
    KeyValue { encryption_key: Option<EncryptionKey> },
    /// StateStore. Table name = `{project}_{stage}_resources`.
    State { table_name: String },
    /// StateStore with Turso remote sync. Table name = `{project}_{stage}_resources`.
    StateRemote { table_name: String },
}

// ============================================================================
// LibsqlStore
// ============================================================================

#[derive(Clone)]
pub struct LibsqlStore {
    conn: Arc<libsql::Connection>,
    db: Option<Arc<libsql::Database>>,
    mode: LibsqlMode,
}

impl LibsqlStore {
    // ========== KV-mode constructors ==========

    /// KV/query/rate-limit/blob store with optional encryption.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub fn new_kv(db_path: &str, encryption_key: Option<EncryptionKey>) -> StorageResult<Self> {
        let path = db_path.to_string();
        let db = exec_future(async move { libsql::Builder::new_local(&path).build().await })?;
        let conn = db.connect().map_err(|e| StorageError::Backend(format!("Connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: None,
            mode: LibsqlMode::KeyValue { encryption_key },
        })
    }

    // ========== State-mode constructors ==========

    /// Local-only StateStore.
    ///
    /// Table name = `{project}_{stage}_resources`.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database cannot be opened.
    pub fn new_state(db_path: &str, project: &str, stage: &str) -> StorageResult<Self> {
        if let Some(parent) = Path::new(db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path = db_path.to_string();
        let table_name = format!(
            "{}_{}_resources",
            project.replace(['-', ' '], "_"),
            stage.replace(['-', ' '], "_"),
        );
        let db = exec_future(async move { libsql::Builder::new_local(&path).build().await })?;
        let conn = db.connect().map_err(|e| StorageError::Backend(format!("Connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: None,
            mode: LibsqlMode::State { table_name },
        })
    }

    /// StateStore with Turso remote sync.
    ///
    /// Table name = `{project}_{stage}_resources`.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the remote connection fails.
    pub fn new_state_remote(
        local_path: &str,
        turso_url: &str,
        auth_token: &str,
        project: &str,
        stage: &str,
    ) -> StorageResult<Self> {
        if let Some(parent) = Path::new(local_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path = local_path.to_string();
        let url = turso_url.to_string();
        let token = auth_token.to_string();
        let table_name = format!(
            "{}_{}_resources",
            project.replace(['-', ' '], "_"),
            stage.replace(['-', ' '], "_"),
        );
        let db = exec_future(async move {
            libsql::Builder::new_remote_replica(path, url, token).build().await
        })?;
        let conn = db.connect().map_err(|e| StorageError::Backend(format!("Connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            db: Some(Arc::new(db)),
            mode: LibsqlMode::StateRemote { table_name },
        })
    }

    // ========== Environment constructors ==========

    /// KV store from environment. Uses `DEPLOYMENT_STATE_DB` or default path.
    pub fn from_env_kv() -> StorageResult<Self> {
        let path = std::env::var("DEPLOYMENT_STATE_DB")
            .unwrap_or_else(|_| ".deployment/state.db".to_string());
        Self::new_kv(&path, None)
    }

    /// StateStore from environment.
    ///
    /// Priority:
    ///   1. `TURSO_DATABASE_URL` + `TURSO_AUTH_TOKEN` → remote Turso
    ///   2. `LIBSQL_TURSO_URL` + `LIBSQL_TURSO_TOKEN` → embedded replica
    ///   3. `LIBSQL_LOCAL_PATH` or `DEPLOYMENT_STATE_DB` → local SQLite
    ///   4. Default: `{project_dir}/.deployment/state.db` → local SQLite
    pub fn from_env_state(project_dir: &Path, project: &str, stage: &str) -> StorageResult<Self> {
        // Remote Turso
        if let (Ok(url), Ok(token)) = (
            std::env::var("TURSO_DATABASE_URL"),
            std::env::var("TURSO_AUTH_TOKEN"),
        ) {
            let local = std::env::var("TURSO_LOCAL_REPLICA")
                .unwrap_or_else(|_| ".deployment/turso_replica.db".to_string());
            return Self::new_state_remote(&local, &url, &token, project, stage);
        }

        // Embedded replica via libsql env
        if let (Ok(url), Ok(token)) = (
            std::env::var("LIBSQL_TURSO_URL"),
            std::env::var("LIBSQL_TURSO_TOKEN"),
        ) {
            let local = std::env::var("LIBSQL_LOCAL_PATH")
                .unwrap_or_else(|_| ".deployment/libsql.db".to_string());
            return Self::new_state_remote(&local, &url, &token, project, stage);
        }

        // Local-only
        let path = std::env::var("LIBSQL_LOCAL_PATH")
            .or_else(|_| std::env::var("DEPLOYMENT_STATE_DB"))
            .unwrap_or_else(|_| format!("{}/.deployment/state.db", project_dir.display()));
        Self::new_state(&path, project, stage)
    }

    // ========== Shared helpers ==========

    fn state_table(&self) -> &str {
        match &self.mode {
            LibsqlMode::State { table_name } | LibsqlMode::StateRemote { table_name } => table_name,
            LibsqlMode::KeyValue { .. } => unreachable!("state_table called on KV mode"),
        }
    }

    fn to_libsql_params(params: &[DataValue]) -> Vec<libsql::Value> {
        params.iter().map(|v| match v {
            DataValue::Null => libsql::Value::Null,
            DataValue::Integer(i) => libsql::Value::Integer(*i),
            DataValue::Real(r) => libsql::Value::Real(*r),
            DataValue::Text(s) => libsql::Value::Text(s.clone()),
            DataValue::Blob(b) => libsql::Value::Blob(b.clone()),
        }).collect()
    }

    fn libsql_row_to_sql_row(row: &libsql::Row, column_count: i32) -> StorageResult<SqlRow> {
        let mut columns = Vec::with_capacity(column_count.unsigned_abs() as usize);
        for i in 0..column_count {
            let name = format!("col{i}");
            let value = Self::libsql_value_to_data_value(row.get_value(i)?);
            columns.push((name, value));
        }
        Ok(SqlRow::new(columns))
    }

    fn libsql_value_to_data_value(value: libsql::Value) -> DataValue {
        match value {
            libsql::Value::Null => DataValue::Null,
            libsql::Value::Integer(i) => DataValue::Integer(i),
            libsql::Value::Real(r) => DataValue::Real(r),
            libsql::Value::Text(s) => DataValue::Text(s),
            libsql::Value::Blob(b) => DataValue::Blob(b),
        }
    }

    fn maybe_encrypt(&self, json_str: &str) -> StorageResult<String> {
        match &self.mode {
            LibsqlMode::KeyValue { encryption_key: Some(key) } => {
                let encrypted = encrypt(key, json_str.as_bytes())?;
                Ok(STANDARD.encode(&encrypted))
            }
            _ => Ok(json_str.to_string()),
        }
    }

    fn maybe_decrypt(&self, stored_value: &str) -> StorageResult<String> {
        match &self.mode {
            LibsqlMode::KeyValue { encryption_key: Some(key) } => {
                let encrypted = STANDARD.decode(stored_value)
                    .map_err(|e| StorageError::Encryption(format!("Base64 decode failed: {e}")))?;
                let decrypted = decrypt(key, &encrypted)?;
                String::from_utf8(decrypted)
                    .map_err(|e| StorageError::Encryption(format!("Invalid UTF-8 in decrypted data: {e}")))
            }
            _ => Ok(stored_value.to_string()),
        }
    }

    fn wrap_value<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }

    fn wrap_vec<T: Send + 'static>(vals: Vec<T>) -> StorageItemStream<'static, T> {
        Box::new(vals.into_iter().map(|v| Stream::Next(Ok(v))))
    }

    fn wrap_value_state<T: Send + 'static>(val: T) -> StateStoreStream<T> {
        Box::new(std::iter::once(ThreadedValue::Value(Ok(val))))
    }

    fn wrap_vec_state<T: Send + 'static>(vals: Vec<T>) -> StateStoreStream<T> {
        Box::new(vals.into_iter().map(|v| ThreadedValue::Value(Ok(v))))
    }

    // ========== KV init ==========

    pub fn init_kv(&self) -> StorageResult<()> {
        let schema_sql = r"
            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            );
            CREATE INDEX IF NOT EXISTS idx_kv_store_key ON kv_store(key);
        ";
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(schema_sql).await })?;
        Ok(())
    }

    // ========================================================================
    // Async-First internal methods (source of truth for all KV/blob/SQL ops)
    // ========================================================================

    async fn get_async_internal<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        let storage = self.clone();

        let opt = conn
            .prepare("SELECT value FROM kv_store WHERE key = ?")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .query([key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        match opt {
            Some(row) => {
                let stored: String = row
                    .get(0)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let json_str = storage
                    .maybe_decrypt(&stored)
                    .map_err(|e| StorageError::Encryption(e.to_string()))?;
                let value: V = serde_json::from_str(&json_str)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set_async_internal<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()> {
        let serialized = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let stored_value = self.maybe_encrypt(&serialized)?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        conn.execute(
            "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
            [key.clone(), stored_value.clone(), stored_value],
        )
        .await
        .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn delete_async_internal(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn exists_async_internal(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        let exists = conn
            .prepare("SELECT 1 FROM kv_store WHERE key = ? LIMIT 1")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .query([key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .is_some();
        Ok(exists)
    }

    async fn query_async_internal(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>> {
        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let mut stmt = conn
            .prepare(&sql)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        let mut rows = stmt
            .query(libsql_params)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            results.push(Self::libsql_row_to_sql_row(&row, row.column_count())?);
        }
        Ok(results)
    }

    async fn execute_async_internal(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute(&sql, libsql_params)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))
    }

    async fn execute_batch_async_internal(&self, sql: &str) -> StorageResult<()> {
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute_batch(&sql)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))
    }

    async fn check_rate_limit_async_internal(&self, key: &str, max_count: u32, window_seconds: u64) -> StorageResult<bool> {
        let create_table = "CREATE TABLE IF NOT EXISTS rate_limits (key TEXT PRIMARY KEY, count INTEGER NOT NULL, window_start INTEGER NOT NULL)";
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(create_table).await })?;

        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let window_start = now - window_seconds;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let opt = conn
            .prepare("SELECT count, window_start FROM rate_limits WHERE key = ?")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .query([key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        match opt {
            Some(row) => {
                let count: i64 = row.get(0).map_err(|e| StorageError::Backend(e.to_string()))?;
                let stored: i64 = row.get(1).map_err(|e| StorageError::Backend(e.to_string()))?;
                if stored.cast_unsigned() < window_start { Ok(true) } else { Ok(count < i64::from(max_count)) }
            }
            None => Ok(true),
        }
    }

    async fn record_rate_limit_async_internal(&self, key: &str) -> StorageResult<u32> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs().cast_signed();
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        conn.execute(
            "INSERT INTO rate_limits (key, count, window_start) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start",
            [key.clone(), now.to_string()],
        )
        .await
        .map_err(|e| StorageError::Backend(e.to_string()))?;

        let opt = conn
            .prepare("SELECT count FROM rate_limits WHERE key = ?")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .query([key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        match opt {
            Some(row) => {
                let c: i64 = row.get(0).map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok(u32::try_from(c).unwrap_or(1))
            }
            None => Ok(1),
        }
    }

    async fn reset_rate_limit_async_internal(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM rate_limits WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn put_blob_async_internal(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let create_table = "CREATE TABLE IF NOT EXISTS blobs (key TEXT PRIMARY KEY, data BLOB NOT NULL, size INTEGER NOT NULL, created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))";
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(create_table).await })?;

        let encoded = STANDARD.encode(data);
        let key = key.to_string();
        let size_str = data.len().to_string();
        let conn = Arc::clone(&self.conn);

        conn.execute(
            "INSERT INTO blobs (key, data, size, updated_at) VALUES (?, ?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET data = ?, size = ?, updated_at = strftime('%s', 'now') * 1000",
            [key.clone(), encoded.clone(), size_str.clone(), encoded, size_str],
        )
        .await
        .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn get_blob_async_internal(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let opt = conn
            .prepare("SELECT data FROM blobs WHERE key = ?")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .query([key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        match opt {
            Some(row) => {
                let encoded: String = row.get(0).map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok(Some(STANDARD.decode(&encoded)
                    .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?))
            }
            None => Ok(None),
        }
    }

    async fn delete_blob_async_internal(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM blobs WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn blob_exists_async_internal(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        let exists = conn
            .prepare("SELECT 1 FROM blobs WHERE key = ? LIMIT 1")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .query([key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .next()
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?
            .is_some();
        Ok(exists)
    }

    /// Helper: wrap an async result into a `StorageItemStream` via `from_future`.
    fn wrap_async<T: Send + 'static>(
        future: impl std::future::Future<Output = StorageResult<T>> + Send + 'static,
    ) -> StorageResult<StorageItemStream<'static, T>> {
        use foundation_core::valtron::execute;

        let task = from_future(future);
        let stream = execute(task, None)
            .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;

        Ok(Box::new(
            stream
                .map_circuit(|item| match item {
                    Stream::Next(result) => match result {
                        Ok(v) => ShortCircuit::Continue(Stream::Next(Ok(v))),
                        Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(e))),
                    },
                    _ => ShortCircuit::Continue(Stream::Ignore),
                })
                .map_pending(|_| ()),
        ))
    }
}

// ===========================================================================
// KeyValueStore
// ===========================================================================

impl KeyValueStore for LibsqlStore {
    fn get<'a, V: DeserializeOwned + Send + 'static>(&'a self, key: &str) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.get_async_internal::<V>(&key).await })
    }

    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.set_async_internal(&key, value).await })
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.delete_async_internal(&key).await })
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.exists_async_internal(&key).await })
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let (sql, param): (&str, String) = match prefix {
            Some(p) => ("SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key", format!("{p}%")),
            None => ("SELECT key FROM kv_store ORDER BY key", String::new()),
        };
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(move || async move {
            let mut stmt = conn.prepare(sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = if param.is_empty() {
                stmt.query([libsql::Value::Null; 0]).await.map_err(|e| StorageError::Backend(e.to_string()))?
            } else {
                stmt.query([param]).await.map_err(|e| StorageError::Backend(e.to_string()))?
            };
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, |row| {
                row.get::<String>(0).map_err(|e| StorageError::SqlConversion(e.to_string()))
            }))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;

        let stream = iter.map(|tv| match tv {
            ThreadedValue::Value(result) => Stream::Next(result),
            ThreadedValue::Waiting => Stream::Pending(()),
        });
        Ok(Box::new(stream))
    }
}

// ===========================================================================
// QueryStore
// ===========================================================================

impl QueryStore for LibsqlStore {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(move || async move {
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query(libsql_params).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, |row| {
                Self::libsql_row_to_sql_row(row, row.column_count())
            }))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;

        let stream = iter.map(|tv| match tv {
            ThreadedValue::Value(result) => Stream::Next(result),
            ThreadedValue::Waiting => Stream::Pending(()),
        });
        Ok(Box::new(stream))
    }

    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, u64>> {
        let sql = sql.to_string();
        let storage = self.clone();
        let params = params.to_vec();
        Self::wrap_async(async move { storage.execute_async_internal(&sql, &params).await })
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = sql.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.execute_batch_async_internal(&sql).await })
    }
}

// ===========================================================================
// RateLimiterStore
// ===========================================================================

impl RateLimiterStore for LibsqlStore {
    fn check_rate_limit(&self, key: &str, max_count: u32, window_seconds: u64) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move {
            storage.check_rate_limit_async_internal(&key, max_count, window_seconds).await
        })
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move {
            storage.record_rate_limit_async_internal(&key).await
        })
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move {
            storage.reset_rate_limit_async_internal(&key).await
        })
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

impl BlobStore for LibsqlStore {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let data = data.to_vec();
        let storage = self.clone();
        Self::wrap_async(async move { storage.put_blob_async_internal(&key, &data).await })
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.get_blob_async_internal(&key).await })
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.delete_blob_async_internal(&key).await })
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async(async move { storage.blob_exists_async_internal(&key).await })
    }
}

// ===========================================================================
// StateStore
// ===========================================================================

impl StateStore for LibsqlStore {
    fn init(&self) -> Result<(), StorageError> {
        let table = self.state_table();
        let table_sql = state_create_table_sql(table);
        let index_sql = state_create_indexes_sql(table);
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(&format!("{table_sql}\n{index_sql}")).await })?;
        Ok(())
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id FROM {table} ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Null; 0]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, |row| {
                row.get::<String>(0).map_err(|e| StorageError::SqlConversion(e.to_string()))
            }))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let stream = schedule_future(async move {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            let mut stmt = conn.prepare(&sql).await?;
            let mut rows = stmt.query([libsql::Value::Null; 0]).await?;
            let count = rows.next().await?.map_or(Ok(0), |row| {
                row.get::<i64>(0).map(|c| usize::try_from(c).unwrap_or(0))
            })?;
            Ok::<_, libsql::Error>(count)
        })?;
        Ok(to_state_stream(stream))
    }

    fn get(&self, resource_id: &str) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let id = resource_id.to_string();
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let stream = schedule_future(async move {
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} WHERE id = ?");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt.query([id]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            match rows.next().await.map_err(|e| StorageError::Backend(e.to_string()))? {
                Some(row) => Ok(Some(parse_state_row(&row)?)),
                None => Ok(None),
            }
        })?;
        Ok(to_state_stream(stream))
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        if ids.is_empty() { return Ok(Box::new(std::iter::empty())); }
        let owned_ids: Vec<String> = ids.iter().map(|s| (*s).to_string()).collect();
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();

        let iter = run_future_iter(move || async move {
            let placeholders = owned_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} WHERE id IN ({placeholders})");
            let params: Vec<libsql::Value> = owned_ids.iter().map(|id| libsql::Value::Text(id.clone())).collect();
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query(params).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_state_row))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Null; 0]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_state_row))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn set(&self, _resource_id: &str, state: &ResourceState) -> Result<StateStoreStream<()>, StorageError> {
        let params = state_to_params(state)?;
        let conn = Arc::clone(&self.conn);
        let sql = state_upsert_sql(self.state_table());
        let stream = schedule_future(async move {
            conn.execute(&sql, params).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(())
        })?;
        Ok(to_state_stream(stream))
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let id = resource_id.to_string();
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let stream = schedule_future(async move {
            conn.execute(&format!("DELETE FROM {table} WHERE id = ?"), [id]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(())
        })?;
        Ok(to_state_stream(stream))
    }

    fn list_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<String>, StorageError> {
        let prefix = escape_like(prefix);
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Text(prefix)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, |row| {
                row.get::<String>(0).map_err(|e| StorageError::SqlConversion(e.to_string()))
            }))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn count_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let prefix = escape_like(prefix);
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let stream = schedule_future(async move {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\'");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt.query([libsql::Value::Text(prefix)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let count = rows.next().await.map_err(|e| StorageError::Backend(e.to_string()))?
                .map_or(Ok::<usize, StorageError>(0), |row| {
                    let c: i64 = row.get(0).map_err(|e| StorageError::Backend(e.to_string()))?;
                    Ok(usize::try_from(c).unwrap_or(0))
                })?;
            Ok::<_, StorageError>(count)
        })?;
        Ok(to_state_stream(stream))
    }

    fn all_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let prefix = escape_like(prefix);
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Text(prefix)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_state_row))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn find_by_kind(&self, kind: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let kind = kind.to_string();
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} WHERE kind = ? ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Text(kind)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_state_row))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn find_by_status(&self, status: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let status = status.to_string();
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} WHERE status = ? ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Text(status)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_state_row))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn find_by_provider(&self, provider: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let provider = provider.to_string();
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let iter = run_future_iter(move || async move {
            let sql = format!("SELECT id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at FROM {table} WHERE provider = ? ORDER BY id");
            let mut stmt = conn.prepare(&sql).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query([libsql::Value::Text(provider)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows, parse_state_row))
        }, None, None).map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn delete_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let prefix = escape_like(prefix);
        let conn = Arc::clone(&self.conn);
        let table = self.state_table().to_string();
        let stream = schedule_future(async move {
            let sql = format!("DELETE FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\'");
            let result = conn.execute(&sql, [libsql::Value::Text(prefix)]).await.map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(usize::try_from(result).unwrap_or(0))
        })?;
        Ok(to_state_stream(stream))
    }

    fn sync_remote(&self) -> Result<(), StorageError> {
        if let LibsqlMode::StateRemote { .. } = &self.mode {
            if let Some(ref db) = self.db {
                let db = Arc::clone(db);
                exec_future(async move {
                    db.sync().await.map(|_| ()).map_err(|e| StorageError::Backend(format!("sync failed: {e}")))
                })?;
            }
        }
        Ok(())
    }
}

// ===========================================================================
// Async trait implementations — direct calls to _async_internal methods.
// ===========================================================================

#[async_trait::async_trait(?Send)]
impl AsyncKeyValueStore for LibsqlStore {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        self.get_async_internal::<V>(key).await
    }
    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        self.set_async_internal(key, value).await
    }
    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        self.delete_async_internal(key).await
    }
    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        self.exists_async_internal(key).await
    }
    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        collect_result(<Self as KeyValueStore>::list_keys(self, prefix)?)
            .into_iter().collect()
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for LibsqlStore {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>> {
        self.query_async_internal(sql, params).await
    }
    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.execute_async_internal(sql, params).await
    }
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.execute_batch_async_internal(sql).await
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncRateLimiterStore for LibsqlStore {
    async fn check_rate_limit_async(&self, key: &str, max_count: u32, window_seconds: u64) -> StorageResult<bool> {
        self.check_rate_limit_async_internal(key, max_count, window_seconds).await
    }
    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        self.record_rate_limit_async_internal(key).await
    }
    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        self.reset_rate_limit_async_internal(key).await
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for LibsqlStore {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_blob_async_internal(key, data).await
    }
    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_blob_async_internal(key).await
    }
    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        self.delete_blob_async_internal(key).await
    }
    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        self.blob_exists_async_internal(key).await
    }
}
