//! Turso storage backend implementation.
//!
//! All business logic lives in async methods. Sync methods wrap async via
//! Valtron's `run_future_iter` (for !Send streams) or `from_future` (for
//! single-value operations). Multi-row queries return `AsyncQueryStream`
//! for true row-by-row async iteration.

use crate::core::crypto::{decrypt, encrypt, EncryptionKey};
use crate::core::errors::StorageResult;
use async_stream::try_stream;
use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::{
    collect_one, execute, from_future, run_future_iter, ShortCircuit, Stream, StreamIteratorExt,
    ThreadedValue,
};
use futures_core::Stream as AsyncStream;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use turso::Builder;

use crate::core::errors::StorageError;

/// One-shot blocking bridge for initialization and migrations.
fn exec_future<T, E, F>(future: F) -> StorageResult<T>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    F::Output: Send + 'static,
    T: Send + 'static,
    E: Into<StorageError> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron execution failed: {e}")))?;
    let result: Result<Option<T>, StorageError> = collect_one(stream)
        .map(|r| r.map_err(Into::into))
        .transpose();
    result?.ok_or_else(|| StorageError::Generic("No result from future execution".into()))
}
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncListStream, AsyncListStreamIterator,
    AsyncQueryStream, AsyncQueryStreamIterator, AsyncQueryStore, AsyncRateLimiterStore,
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// Turso storage backend with optional encryption support.
///
/// When an encryption key is provided, all values are encrypted at rest
/// using ChaCha20-Poly1305 before being stored in the database.
#[derive(Clone)]
pub struct TursoStorage {
    conn: Arc<turso::Connection>,
    encryption_key: Option<EncryptionKey>,
}

impl TursoStorage {
    /// Create a new Turso storage connection without encryption.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub fn new(url: &str) -> StorageResult<Self> {
        Self::with_encryption(url, None)
    }

    /// Create a new Turso storage connection with optional encryption.
    ///
    /// When an encryption key is provided, all values are encrypted at rest
    /// using ChaCha20-Poly1305 before being stored in the database.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub fn with_encryption(
        url: &str,
        encryption_key: Option<EncryptionKey>,
    ) -> StorageResult<Self> {
        let url = url.to_string();
        let db: turso::Database =
            exec_future(async move { Builder::new_local(&url).build().await })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Backend(format!("Connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
            encryption_key,
        })
    }

    /// Initialize the database schema.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if schema creation fails.
    pub fn init_schema(&self) -> StorageResult<()> {
        // Apply the full canonical migration set (kv_store via 001 … documents +
        // promoted columns via 020/021). Previously this hardcoded only
        // `kv_store`/`_migrations`, so every other table — including `documents`
        // — was never created on the native sync path. The runner is idempotent
        // (each migration is `IF NOT EXISTS` and tracked in `_migrations`).
        crate::core::schema::MigrationRunner::new(crate::core::schema::MIGRATIONS).run(self)?;
        Ok(())
    }

    /// Run database migrations.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if migration execution fails.
    pub fn migrate(&self, migrations: &[(&str, &str)]) -> StorageResult<()> {
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS _migrations (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
                )",
            )
            .await
        })?;

        for (id, sql) in migrations {
            let id = id.to_string();
            let sql = sql.to_string();
            let conn = Arc::clone(&self.conn);
            let _applied: bool = exec_future(async move {
                let mut stmt = conn
                    .prepare("SELECT 1 FROM _migrations WHERE id = ?")
                    .await?;
                let mut rows = stmt.query([id.clone()]).await?;
                let exists = rows.next().await?.is_some();
                if exists {
                    Ok::<_, turso::Error>(false)
                } else {
                    conn.execute_batch(&sql).await?;
                    conn.execute(
                        "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                        [id.clone(), id],
                    )
                    .await?;
                    Ok::<_, turso::Error>(true)
                }
            })?;
        }

        Ok(())
    }

    /// Convert crate-owned [`DataValue`] slice to `turso::Value` Vec.
    fn to_turso_params(params: &[DataValue]) -> Vec<turso::Value> {
        params.iter().map(Self::data_value_to_turso).collect()
    }

    /// Convert a single [`DataValue`] to `turso::Value`.
    fn data_value_to_turso(value: &DataValue) -> turso::Value {
        match value {
            DataValue::Null => turso::Value::Null,
            DataValue::Integer(i) => turso::Value::Integer(*i),
            DataValue::Real(r) => turso::Value::Real(*r),
            DataValue::Text(s) => turso::Value::Text(s.clone()),
            DataValue::Blob(b) => turso::Value::Blob(b.clone()),
        }
    }

    /// Convert `turso::Row` to crate-owned [`SqlRow`].
    fn turso_row_to_sql_row(row: &turso::Row, column_count: i32) -> StorageResult<SqlRow> {
        let mut columns = Vec::with_capacity(column_count.unsigned_abs() as usize);
        for i in 0..column_count {
            let name = format!("col{i}");
            #[allow(clippy::cast_sign_loss)]
            let value = Self::turso_value_to_data_value(row.get_value(i as usize)?);
            columns.push((name, value));
        }
        Ok(SqlRow::new(columns))
    }

    /// Convert `turso::Value` to crate-owned [`DataValue`].
    fn turso_value_to_data_value(value: turso::Value) -> DataValue {
        match value {
            turso::Value::Null => DataValue::Null,
            turso::Value::Integer(i) => DataValue::Integer(i),
            turso::Value::Real(r) => DataValue::Real(r),
            turso::Value::Text(s) => DataValue::Text(s),
            turso::Value::Blob(b) => DataValue::Blob(b),
        }
    }

    /// Encrypt a JSON-serialized value if encryption is enabled.
    fn maybe_encrypt(&self, json_str: &str) -> StorageResult<String> {
        match &self.encryption_key {
            Some(key) => {
                let encrypted = encrypt(key, json_str.as_bytes())?;
                Ok(STANDARD.encode(&encrypted))
            }
            None => Ok(json_str.to_string()),
        }
    }

    /// Decrypt a value if encryption is enabled.
    fn maybe_decrypt(&self, stored_value: &str) -> StorageResult<String> {
        match &self.encryption_key {
            Some(key) => {
                let encrypted = STANDARD
                    .decode(stored_value)
                    .map_err(|e| StorageError::Encryption(format!("Base64 decode failed: {e}")))?;
                let decrypted = decrypt(key, &encrypted)?;
                String::from_utf8(decrypted).map_err(|e| {
                    StorageError::Encryption(format!("Invalid UTF-8 in decrypted data: {e}"))
                })
            }
            None => Ok(stored_value.to_string()),
        }
    }

    // ========================================================================
    // Async-First internal methods (source of truth for all operations)
    // ========================================================================

    /// Async KV get: select → decrypt → deserialize.
    async fn get_async_internal<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
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
                let stored_value: String = row
                    .get(0)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let json_str = storage
                    .maybe_decrypt(&stored_value)
                    .map_err(|e| StorageError::Encryption(e.to_string()))?;
                let value: V = serde_json::from_str(&json_str)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Async KV set: serialize → encrypt → upsert.
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

    /// Async KV delete.
    async fn delete_async_internal(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Async KV exists.
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

    /// Async blob put.
    async fn put_blob_async_internal(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let encoded = STANDARD.encode(data);
        let key = key.to_string();
        #[allow(clippy::cast_possible_wrap)]
        let size = data.len() as i64;
        let size_str = size.to_string();
        let conn = Arc::clone(&self.conn);

        conn.execute(
            "INSERT INTO blobs (key, data, size, updated_at) VALUES (?, ?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET data = ?, size = ?, updated_at = strftime('%s', 'now') * 1000",
            [key.clone(), encoded.clone(), size_str.clone(), encoded, size_str],
        )
        .await
        .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Async blob get.
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
                let encoded: String = row
                    .get(0)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok(Some(
                    STANDARD
                        .decode(&encoded)
                        .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?,
                ))
            }
            None => Ok(None),
        }
    }

    /// Async blob delete.
    async fn delete_blob_async_internal(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM blobs WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Async blob exists.
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

    /// Async rate limit check.
    async fn check_rate_limit_async_internal(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let create_table = r"
            CREATE TABLE IF NOT EXISTS rate_limits (
                key TEXT PRIMARY KEY,
                count INTEGER NOT NULL,
                window_start INTEGER NOT NULL
            )
        ";
        let conn = Arc::clone(&self.conn);
        conn.execute_batch(create_table).await
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
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
                let count: i64 = row
                    .get(0)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let stored_window_start: i64 = row
                    .get(1)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                if stored_window_start.cast_unsigned() < window_start {
                    Ok(true)
                } else {
                    Ok(count < i64::from(max_count))
                }
            }
            None => Ok(true),
        }
    }

    /// Async rate limit record.
    async fn record_rate_limit_async_internal(&self, key: &str) -> StorageResult<u32> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .cast_signed();
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
                let count: i64 = row
                    .get(0)
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok(u32::try_from(count).map_err(|e| {
                    StorageError::Backend(format!("Cannot convert count to u32: {e}"))
                })?)
            }
            None => Ok(1),
        }
    }

    /// Async rate limit reset.
    async fn reset_rate_limit_async_internal(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM rate_limits WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Creates a lazy async stream that yields SQL rows one at a time.
    ///
    /// NOTE: This is a **sync function** returning a stream — it does NOT block.
    /// The `.await` calls inside `try_stream!` are lazy: they only execute when
    /// the stream is polled. This function just constructs and returns the
    /// generator state machine.
    ///
    /// Callers use the stream in two ways:
    /// - **Async** (`query_async`): wraps it in `AsyncQueryStream`, caller polls directly
    /// - **Sync** (`query`): sends it through `run_future_iter`'s worker thread,
    ///   where `block_on` polls each row on demand
    fn query_rows_stream(
        conn: Arc<turso::Connection>,
        sql: String,
        params: Vec<turso::Value>,
    ) -> impl AsyncStream<Item = StorageResult<SqlRow>> + Send {
        try_stream! {
            let mut stmt = conn.prepare(&sql).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt.query(params).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;

            while let Some(row) = rows.next().await
                .map_err(|e| StorageError::Backend(e.to_string()))?
            {
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                yield Self::turso_row_to_sql_row(&row, row.column_count() as i32)?;
            }
        }
    }

    /// Creates a lazy async stream that yields KV keys one at a time.
    ///
    /// Same pattern as `query_rows_stream`: sync function returning a lazy stream.
    /// The `.await` calls inside are deferred until the stream is polled.
    ///
    /// Used by:
    /// - `list_keys_async` (async) — wraps in `AsyncListStream`
    /// - `list_keys` (sync) — bridges via `run_future_iter` + `AsyncListStreamIterator`
    fn list_keys_stream(
        conn: Arc<turso::Connection>,
        sql: String,
        param: turso::Value,
    ) -> impl AsyncStream<Item = StorageResult<String>> + Send {
        try_stream! {
            let mut stmt = conn.prepare(&sql).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt.query([param]).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;

            while let Some(row) = rows.next().await
                .map_err(|e| StorageError::Backend(e.to_string()))?
            {
                yield row.get::<String>(0)
                    .map_err(|e| StorageError::SqlConversion(e.to_string()))?;
            }
        }
    }

    /// Async execute (returns rows affected).
    async fn execute_async_internal(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<u64> {
        let turso_params = Self::to_turso_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute(&sql, turso_params)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))
    }

    /// Async `execute_batch`.
    async fn execute_batch_async_internal(&self, sql: &str) -> StorageResult<()> {
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute_batch(&sql)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))
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
                        Err(e) => {
                            ShortCircuit::ReturnAndStop(Stream::Next(Err(e)))
                        }
                    },
                    _ => ShortCircuit::Continue(Stream::Ignore),
                })
                .map_pending(|_| ()),
        ))
    }

    /// Helper: wrap an async result into a direct value via valtron.
    fn wrap_async_value<T: Send + 'static>(
        future: impl std::future::Future<Output = StorageResult<T>> + Send + 'static,
    ) -> StorageResult<T> {
        use foundation_core::valtron::{execute, collect_one};

        let task = from_future(future);
        let stream = execute(task, None)
            .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;

        collect_one(stream)
            .transpose()
            .map_err(|e| StorageError::Backend(format!("Execution failed: {e}")))?
            .ok_or_else(|| StorageError::Backend("No result from async operation".to_string()))
    }
}

impl KeyValueStore for TursoStorage {
    fn get<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.get_async_internal::<V>(&key).await
        })
    }

    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.set_async_internal(&key, value).await
        })
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.delete_async_internal(&key).await
        })
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.exists_async_internal(&key).await
        })
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let this = self.clone();
        let prefix = prefix.map(String::from);

        let iter = run_future_iter(move || async move {
            let stream = this.list_keys_async(prefix.as_deref()).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(AsyncListStreamIterator::new(stream))
        }, None, None)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let stream = iter.map(|threaded_value| match threaded_value {
            ThreadedValue::Value(result) => Stream::Next(result),
            ThreadedValue::Waiting => Stream::Pending(()),
        });

        Ok(Box::new(stream))
    }
}

impl QueryStore for TursoStorage {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let this = self.clone();
        let sql = sql.to_string();
        let params = params.to_vec();

        let iter = run_future_iter(move || async move {
            let stream = this.query_async(&sql, &params).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(AsyncQueryStreamIterator::new(stream))
        }, None, None)
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        let stream = iter.map(|threaded_value| match threaded_value {
            ThreadedValue::Value(result) => Stream::Next(result),
            ThreadedValue::Waiting => Stream::Pending(()),
        });

        Ok(Box::new(stream))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<u64> {
        let sql = sql.to_string();
        let storage = self.clone();
        let params = params.to_vec();
        Self::wrap_async_value(async move {
            storage.execute_async_internal(&sql, &params).await
        })
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        let sql = sql.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.execute_batch_async_internal(&sql).await
        })
    }
}

impl RateLimiterStore for TursoStorage {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.check_rate_limit_async_internal(&key, max_count, window_seconds).await
        })
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<u32> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.record_rate_limit_async_internal(&key).await
        })
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.reset_rate_limit_async_internal(&key).await
        })
    }
}

impl BlobStore for TursoStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let key = key.to_string();
        let data = data.to_vec();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.put_blob_async_internal(&key, &data).await
        })
    }

    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.get_blob_async_internal(&key).await
        })
    }

    fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.delete_blob_async_internal(&key).await
        })
    }

    fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        let storage = self.clone();
        Self::wrap_async_value(async move {
            storage.blob_exists_async_internal(&key).await
        })
    }
}

// ===========================================================================
// Async trait implementations — direct calls to _async_internal methods.
// ===========================================================================

#[async_trait::async_trait]
impl AsyncKeyValueStore for TursoStorage {
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

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let (sql, param) = match prefix {
            Some(p) => (
                "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key",
                Self::to_turso_params(&[DataValue::Text(format!("{p}%"))]),
            ),
            None => (
                "SELECT key FROM kv_store ORDER BY key",
                Self::to_turso_params(&[DataValue::Null]),
            ),
        };
        let conn = Arc::clone(&self.conn);
        // param is Vec<turso::Value>, we take the first element
        let stream = Self::list_keys_stream(conn, sql.to_string(), param.into_iter().next().unwrap_or(turso::Value::Null));
        Ok(AsyncListStream::new(stream))
    }
}

#[async_trait::async_trait]
impl AsyncQueryStore for TursoStorage {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        let params = Self::to_turso_params(params);
        let stream = Self::query_rows_stream(conn, sql, params);
        Ok(AsyncQueryStream::new(stream))
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.execute_async_internal(sql, params).await
    }

    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.execute_batch_async_internal(sql).await
    }
}

#[async_trait::async_trait]
impl AsyncRateLimiterStore for TursoStorage {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        self.check_rate_limit_async_internal(key, max_count, window_seconds).await
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        self.record_rate_limit_async_internal(key).await
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        self.reset_rate_limit_async_internal(key).await
    }
}

#[async_trait::async_trait]
impl AsyncBlobStore for TursoStorage {
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
