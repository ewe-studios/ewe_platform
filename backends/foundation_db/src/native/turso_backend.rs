//! Turso storage backend implementation.
//!
//! `TursoStorage` implements only the `Async*` storage traits: every operation
//! is a native `async fn`, and multi-row queries return an `AsyncQueryStream`
//! pulled row by row. Sync callers wrap the store in `foundation_db::Blocking`.

use crate::core::crypto::{decrypt, encrypt, EncryptionKey};
use crate::core::errors::{StorageError, StorageResult};
use crate::core::schema::{MigrationRunner, MIGRATIONS};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncListStream, AsyncQueryStore, AsyncQueryStream,
    AsyncRateLimiterStore, DataValue, SqlRow,
};
use async_stream::try_stream;
use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::BlockingFutureExt;
use futures_core::Stream as AsyncStream;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use turso::Builder;

/// Turso storage backend with optional encryption support.
///
/// When an encryption key is provided, all values are encrypted at rest
/// using ChaCha20-Poly1305 before being stored in the database.
#[derive(Clone)]
pub struct TursoStorage {
    conn: Arc<turso::Connection>,
    encryption_key: Option<EncryptionKey>,
}

/// An in-memory database with the full schema migrated (`kv_store`, `documents`,
/// …): nothing is persisted, so it suits tests and throwaway sessions. Use
/// [`TursoStorage::new`] with a file path for a store that survives restarts.
///
/// `Default` is a synchronous trait, so this is a sync boundary: it waits for
/// [`TursoStorage::new`] and [`TursoStorage::init_schema`] on the valtron
/// executor with `.block()`. In async code, call those two directly instead.
///
/// # Panics
///
/// If the in-memory database can't be opened or migrated, which only happens
/// when the executor isn't available or `SQLite` itself fails.
impl Default for TursoStorage {
    fn default() -> Self {
        async {
            let storage = Self::new(":memory:")
                .await
                .expect("TursoStorage::default: failed to open an in-memory database");
            storage
                .init_schema()
                .await
                .expect("TursoStorage::default: failed to migrate the in-memory database");
            storage
        }
        .block()
    }
}

impl TursoStorage {
    /// Create a new Turso storage connection without encryption.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub async fn new(url: &str) -> StorageResult<Self> {
        Self::with_encryption(url, None).await
    }

    /// Create a new Turso storage connection with optional encryption.
    ///
    /// When an encryption key is provided, all values are encrypted at rest
    /// using ChaCha20-Poly1305 before being stored in the database.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub async fn with_encryption(
        url: &str,
        encryption_key: Option<EncryptionKey>,
    ) -> StorageResult<Self> {
        let db: turso::Database = Builder::new_local(url).build().await?;
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
    pub async fn init_schema(&self) -> StorageResult<()> {
        // Apply the full canonical migration set (kv_store via 001 … documents +
        // promoted columns via 020/021). Previously this hardcoded only
        // `kv_store`/`_migrations`, so every other table — including `documents`
        // — was never created on the native sync path. The runner is idempotent
        // (each migration is `IF NOT EXISTS` and tracked in `_migrations`).
        MigrationRunner::new(MIGRATIONS)
            .run_async(self)
            .await?;
        Ok(())
    }

    /// Run database migrations.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if migration execution fails.
    pub async fn migrate(&self, migrations: &[(&str, &str)]) -> StorageResult<()> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS _migrations (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
                )",
            )
            .await?;

        for (id, sql) in migrations {
            let mut stmt = self
                .conn
                .prepare("SELECT 1 FROM _migrations WHERE id = ?")
                .await?;
            let mut rows = stmt.query([(*id).to_string()]).await?;
            let exists = rows.next().await?.is_some();
            if !exists {
                self.conn.execute_batch(sql).await?;
                self.conn
                    .execute(
                        "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                        [(*id).to_string(), (*id).to_string()],
                    )
                    .await?;
            }
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

    fn turso_row_to_sql_row(
        row: &turso::Row,
        column_count: i32,
        column_names: &[String],
    ) -> StorageResult<SqlRow> {
        let mut columns = Vec::with_capacity(column_count.unsigned_abs() as usize);
        for i in 0..column_count {
            let name = column_names
                .get(i as usize)
                .cloned()
                .unwrap_or_else(|| format!("col{i}"));
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
                core::str::from_utf8(&decrypted)
                    .map(String::from)
                    .map_err(|e| {
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
    /// `query_async` wraps it in an `AsyncQueryStream` that the caller polls.
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
            // Column names live on the result set (Rows), not individual Row values.
            let column_names = rows.column_names();

            while let Some(row) = rows.next().await
                .map_err(|e| StorageError::Backend(e.to_string()))?
            {
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                yield Self::turso_row_to_sql_row(
                    &row,
                    row.column_count() as i32,
                    &column_names,
                )?;
            }
        }
    }

    /// Creates a lazy async stream that yields KV keys one at a time.
    ///
    /// Same pattern as `query_rows_stream`: sync function returning a lazy stream.
    /// The `.await` calls inside are deferred until the stream is polled.
    ///
    /// `list_keys_async` wraps it in an `AsyncListStream`.
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
