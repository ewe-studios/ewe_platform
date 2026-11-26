//! SQL storage over a Durable Object's SQL database (spec 83 feature 05, T20).
//!
//! WHY: a Cloudflare Durable Object (DO) holds one SQLite database. The agent
//! RPC session stores its history and memory in that database. The session
//! stores need the usual `foundation_db` traits, not the `worker` crate types.
//!
//! WHAT: [`DoSql`] (the SQL engine seam), [`DoSqlStore`] (the
//! [`QueryStore`], [`KeyValueStore`], [`BlobStore`] and [`RateLimiterStore`]
//! implementation) and the table schema that `SqlDocumentStore` and
//! `KvMemoryStore` need.
//!
//! HOW: the DO SQL application programming interface (API) is synchronous: it
//! returns a cursor over the rows. `WorkerDoSql` maps the cursor of the
//! `worker` crate into [`SqlRow`] values. A native host and a test use
//! `SqliteDoSql`, which runs the same statements on an in-memory SQLite
//! database. Both engines follow one contract, so the native test proves the
//! behavior that the DO store has.
//!
//! Limits: a DO SQL column that holds an integer above 2^53 loses precision on
//! read, because the JavaScript runtime represents it as a double (the same
//! limit as D1). Bind such a value as text and `CAST` it, or keep it below
//! 2^53.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncListStream, AsyncQueryStream, BlobStore, DataValue, KeyValueStore, QueryStore,
    RateLimiterStore, SqlRow,
};

/// One SQL engine that a [`DoSqlStore`] runs its statements on.
///
/// The Durable Object engine and the in-memory SQLite test engine implement
/// this seam. A statement that returns rows uses [`query`](Self::query); every
/// other statement uses [`execute`](Self::execute).
#[async_trait]
pub trait DoSql: Send + Sync + 'static {
    /// Run `sql` and return every row. `params` fill the `?` placeholders.
    ///
    /// # Errors
    /// The engine's error, as [`StorageError::Backend`].
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>>;

    /// Run `sql` and return the number of rows that it wrote.
    ///
    /// # Errors
    /// The engine's error, as [`StorageError::Backend`].
    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;
}

/// Storage over one Durable Object's SQL database.
///
/// The `prefix` names the key-value table (`<prefix>_kv`). The documents and
/// rate-limit tables keep their standard names, because one DO holds one
/// session.
pub struct DoSqlStore {
    sql: Arc<dyn DoSql>,
    prefix: String,
    schema_ready: AtomicBool,
}

impl std::fmt::Debug for DoSqlStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoSqlStore")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl DoSqlStore {
    /// A store over `sql` with the key-value table `<prefix>_kv`.
    #[must_use]
    pub fn new(sql: Arc<dyn DoSql>, prefix: &str) -> Self {
        Self {
            sql,
            prefix: prefix.to_string(),
            schema_ready: AtomicBool::new(false),
        }
    }

    /// The key-value table name.
    #[must_use]
    pub fn kv_table(&self) -> String {
        format!("{}_kv", self.prefix)
    }

    /// Create every table that `SqlDocumentStore` and `KvMemoryStore` need.
    ///
    /// Call it once, before the first session opens. The statements are
    /// `IF NOT EXISTS`, so a second call changes nothing.
    ///
    /// # Errors
    /// The engine's error, as [`StorageError::Backend`].
    pub async fn init_schema(&self) -> StorageResult<()> {
        if self.schema_ready.load(Ordering::Acquire) {
            return Ok(());
        }
        let kv = self.kv_table();
        let statements = [
            format!(
                "CREATE TABLE IF NOT EXISTS {kv} (\
                 key TEXT PRIMARY KEY, \
                 value TEXT NOT NULL, \
                 created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000), \
                 updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))"
            ),
            "CREATE TABLE IF NOT EXISTS rate_limits (\
             id TEXT PRIMARY KEY, \
             key TEXT UNIQUE NOT NULL, \
             count INTEGER NOT NULL DEFAULT 0, \
             window_start INTEGER NOT NULL, \
             window_end INTEGER NOT NULL)"
                .to_string(),
            // The final shape of migration 020 plus the promoted columns of
            // migrations 021 and 022. A Durable Object starts with an empty
            // database, so there is no earlier shape to alter.
            "CREATE TABLE IF NOT EXISTS documents (\
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             collection_key TEXT NOT NULL, \
             doc_id TEXT NOT NULL, \
             content TEXT NOT NULL, \
             metadata TEXT NOT NULL DEFAULT '{}', \
             created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
             title TEXT, summary TEXT, record_type TEXT, r2_key TEXT)"
                .to_string(),
            "CREATE INDEX IF NOT EXISTS idx_documents_collection_key ON documents(collection_key)"
                .to_string(),
            "CREATE INDEX IF NOT EXISTS idx_documents_collection_created ON documents(collection_key, created_at)"
                .to_string(),
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_collection_doc_id ON documents(collection_key, doc_id)"
                .to_string(),
            "CREATE INDEX IF NOT EXISTS idx_documents_collection_type ON documents(collection_key, record_type)"
                .to_string(),
        ];
        for statement in statements {
            self.sql.execute(&statement, &[]).await?;
        }
        self.schema_ready.store(true, Ordering::Release);
        Ok(())
    }

    /// Read one value from the key-value table.
    async fn kv_get(&self, key: &str) -> StorageResult<Option<String>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let rows = self
            .sql
            .query(&sql, &[DataValue::Text(key.to_string())])
            .await?;
        match rows.first() {
            Some(row) => row.get_by_name::<String>("value").map(Some),
            None => Ok(None),
        }
    }

    /// Write one value into the key-value table.
    async fn kv_set(&self, key: &str, value: &str) -> StorageResult<()> {
        let table = self.kv_table();
        let sql = format!(
            "INSERT INTO {table} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) \
             ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000"
        );
        self.sql
            .execute(
                &sql,
                &[
                    DataValue::Text(key.to_string()),
                    DataValue::Text(value.to_string()),
                    DataValue::Text(value.to_string()),
                ],
            )
            .await
            .map(|_| ())
    }

    /// Delete one key from the key-value table.
    async fn kv_delete(&self, key: &str) -> StorageResult<()> {
        let sql = format!("DELETE FROM {} WHERE key = ?", self.kv_table());
        self.sql
            .execute(&sql, &[DataValue::Text(key.to_string())])
            .await
            .map(|_| ())
    }

    /// Whether one key exists.
    async fn kv_exists(&self, key: &str) -> StorageResult<bool> {
        let sql = format!("SELECT 1 FROM {} WHERE key = ? LIMIT 1", self.kv_table());
        let rows = self
            .sql
            .query(&sql, &[DataValue::Text(key.to_string())])
            .await?;
        Ok(!rows.is_empty())
    }

    /// List the keys, in order, with an optional prefix.
    async fn kv_list(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        let table = self.kv_table();
        let (sql, params) = match prefix {
            Some(prefix) => (
                format!("SELECT key FROM {table} WHERE key LIKE ? ORDER BY key"),
                vec![DataValue::Text(format!("{prefix}%"))],
            ),
            None => (format!("SELECT key FROM {table} ORDER BY key"), Vec::new()),
        };
        let rows = self.sql.query(&sql, &params).await?;
        rows.iter().map(|row| row.get_by_name("key")).collect()
    }

    /// Put a blob as base64 text in the key-value table.
    async fn blob_put(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        use base64::Engine;
        let value = serde_json::json!({
            "type": "blob",
            "encoding": "base64",
            "data": base64::engine::general_purpose::STANDARD.encode(data),
        })
        .to_string();
        self.kv_set(key, &value).await
    }

    /// Read a blob from the key-value table.
    async fn blob_get(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        use base64::Engine;
        let Some(value) = self.kv_get(key).await? else {
            return Ok(None);
        };
        let parsed: serde_json::Value = serde_json::from_str(&value)
            .map_err(|error| StorageError::Deserialization(error.to_string()))?;
        let encoded = parsed
            .get("data")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                StorageError::SqlConversion("the blob value has no data field".to_string())
            })?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map(Some)
            .map_err(|error| StorageError::Deserialization(error.to_string()))
    }
}

/// Run a batch as separate statements.
///
/// A DO SQL engine runs one statement per call. A migration text uses `;`, so
/// the store splits it. An empty statement is skipped.
async fn execute_statements(sql: &dyn DoSql, batch: &str) -> StorageResult<()> {
    for statement in batch.split(';') {
        let statement = statement.trim();
        if !statement.is_empty() {
            sql.execute(statement, &[]).await?;
        }
    }
    Ok(())
}

#[async_trait]
impl QueryStore for DoSqlStore {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        let rows = self.sql.query(sql, params).await?;
        Ok(AsyncQueryStream::new(futures_lite::stream::iter(
            rows.into_iter().map(Ok),
        )))
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.sql.execute(sql, params).await
    }

    async fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        execute_statements(self.sql.as_ref(), sql).await
    }
}

#[async_trait]
impl KeyValueStore for DoSqlStore {
    async fn get<V: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        match self.kv_get(key).await? {
            Some(value) => serde_json::from_str(&value)
                .map(Some)
                .map_err(|error| StorageError::Deserialization(error.to_string())),
            None => Ok(None),
        }
    }

    async fn set<V: serde::Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> StorageResult<()> {
        let text = serde_json::to_string(&value)
            .map_err(|error| StorageError::Serialization(error.to_string()))?;
        self.kv_set(key, &text).await
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.kv_delete(key).await
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.kv_exists(key).await
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let keys = self.kv_list(prefix).await?;
        Ok(AsyncListStream::new(futures_lite::stream::iter(
            keys.into_iter().map(Ok),
        )))
    }
}

#[async_trait]
impl BlobStore for DoSqlStore {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.blob_put(key, data).await
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.blob_get(key).await
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        self.kv_delete(key).await
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        self.kv_exists(key).await
    }
}

#[cfg(any(
    feature = "turso",
    feature = "d1",
    feature = "libsql",
    all(target_family = "wasm", feature = "wasm-bindgen-storage")
))]
#[async_trait]
impl RateLimiterStore for DoSqlStore {
    async fn check_rate_limit(
        &self,
        ctx: &foundation_context::Ctx,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        crate::core::rate_limit::sql_check(self, ctx, key, max_count, window_seconds).await
    }

    async fn record_rate_limit(
        &self,
        ctx: &foundation_context::Ctx,
        key: &str,
        window_seconds: u64,
    ) -> StorageResult<u32> {
        crate::core::rate_limit::sql_record(self, ctx, key, window_seconds).await
    }

    async fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        crate::core::rate_limit::sql_reset(self, key).await
    }
}

// ---------------------------------------------------------------------------
// The Durable Object engine
// ---------------------------------------------------------------------------

/// The Durable Object SQL engine of the `worker` crate.
#[cfg(all(target_family = "wasm", feature = "workers-rs"))]
pub struct WorkerDoSql {
    sql: worker::SqlStorage,
}

#[cfg(all(target_family = "wasm", feature = "workers-rs"))]
impl std::fmt::Debug for WorkerDoSql {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerDoSql").finish_non_exhaustive()
    }
}

#[cfg(all(target_family = "wasm", feature = "workers-rs"))]
impl WorkerDoSql {
    /// A store over the SQL database of one Durable Object.
    #[must_use]
    pub fn new(sql: worker::SqlStorage) -> Self {
        Self { sql }
    }

    fn bindings(params: &[DataValue]) -> Vec<worker::SqlStorageValue> {
        params
            .iter()
            .map(|value| match value {
                DataValue::Null => worker::SqlStorageValue::Null,
                DataValue::Integer(number) => worker::SqlStorageValue::Integer(*number),
                DataValue::Real(number) => worker::SqlStorageValue::Float(*number),
                DataValue::Text(text) => worker::SqlStorageValue::String(text.clone()),
                DataValue::Blob(bytes) => worker::SqlStorageValue::Blob(bytes.clone()),
            })
            .collect()
    }
}

#[cfg(all(target_family = "wasm", feature = "workers-rs"))]
fn data_value(value: worker::SqlStorageValue) -> DataValue {
    use worker::SqlStorageValue;
    match value {
        SqlStorageValue::Null => DataValue::Null,
        SqlStorageValue::Boolean(flag) => DataValue::Integer(i64::from(flag)),
        SqlStorageValue::Integer(number) => DataValue::Integer(number),
        SqlStorageValue::Float(number) => DataValue::Real(number),
        SqlStorageValue::String(text) => DataValue::Text(text),
        SqlStorageValue::Blob(bytes) => DataValue::Blob(bytes),
    }
}

#[cfg(all(target_family = "wasm", feature = "workers-rs"))]
#[async_trait]
impl DoSql for WorkerDoSql {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>> {
        let cursor = self
            .sql
            .exec(sql, Self::bindings(params))
            .map_err(|error| {
                StorageError::Backend(format!("Durable Object SQL failed: {error}"))
            })?;
        let columns = cursor.column_names();
        let mut rows = Vec::new();
        for row in cursor.raw() {
            let values = row.map_err(|error| {
                StorageError::Backend(format!("Durable Object SQL row failed: {error}"))
            })?;
            let cells: Vec<(String, DataValue)> = columns
                .iter()
                .cloned()
                .zip(values.into_iter().map(data_value))
                .collect();
            rows.push(SqlRow::new(cells));
        }
        Ok(rows)
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let cursor = self
            .sql
            .exec(sql, Self::bindings(params))
            .map_err(|error| {
                StorageError::Backend(format!("Durable Object SQL failed: {error}"))
            })?;
        // Read the cursor to its end, so the statement has run.
        for row in cursor.raw() {
            row.map_err(|error| {
                StorageError::Backend(format!("Durable Object SQL row failed: {error}"))
            })?;
        }
        Ok(u64::try_from(cursor.rows_written()).unwrap_or(u64::MAX))
    }
}

// ---------------------------------------------------------------------------
// The native engine: in-memory SQLite with the same contract
// ---------------------------------------------------------------------------

/// The SQL engine over a native SQLite store.
///
/// A test and a native host use it to exercise the behavior of the Durable
/// Object store without `Miniflare`. It runs the same statements on the same
/// SQLite engine.
#[cfg(all(not(target_family = "wasm"), feature = "turso"))]
pub struct SqliteDoSql {
    storage: crate::native::TursoStorage,
}

#[cfg(all(not(target_family = "wasm"), feature = "turso"))]
impl std::fmt::Debug for SqliteDoSql {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteDoSql").finish_non_exhaustive()
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "turso"))]
impl SqliteDoSql {
    /// A fresh in-memory database, with the core schema migrated.
    ///
    /// # Errors
    /// The SQLite error, as [`StorageError`].
    pub async fn in_memory(ctx: foundation_context::Ctx) -> StorageResult<Self> {
        Ok(Self {
            storage: crate::native::TursoStorage::in_memory(ctx).await?,
        })
    }

    /// A database over an existing store, for a test that prepares the schema.
    #[must_use]
    pub fn new(storage: crate::native::TursoStorage) -> Self {
        Self { storage }
    }
}

#[cfg(all(not(target_family = "wasm"), feature = "turso"))]
#[async_trait]
impl DoSql for SqliteDoSql {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>> {
        self.storage.query(sql, params).await?.collect_all().await
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.storage.execute(sql, params).await
    }
}
