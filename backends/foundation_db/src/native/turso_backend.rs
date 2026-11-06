//! Turso storage backend implementation.
//!
//! `TursoStorage` implements only the `Async*` storage traits: every operation
//! is a native `async fn`, and multi-row queries return an `AsyncQueryStream`
//! pulled row by row.

use crate::core::crypto::{decrypt, encrypt, EncryptionKey};
use crate::core::errors::{StorageError, StorageResult};
use crate::core::rate_limit;
use crate::core::schema::{MigrationRunner, MIGRATIONS};
use crate::core::storage_provider::{
    AsyncListStream, AsyncQueryStream, BlobStore, DataValue, KeyValueStore, QueryStore,
    RateLimiterStore, SqlRow,
};
use crate::native::conn_gate::{is_read_only_sql, ConnGate, ReadGuard, WriteGuard};
use async_stream::try_stream;
use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::Clock;
use futures_core::Stream as AsyncStream;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use turso::Builder;

/// Turso storage backend with optional encryption support.
///
/// When an encryption key is provided, all values are encrypted at rest
/// using ChaCha20-Poly1305 before being stored in the database.
///
/// The store keeps one connection and an async read/write lock (shared by
/// clones): reads run together; a write, a raw statement or a batch (`BEGIN` to
/// `COMMIT`/`ROLLBACK`) has the connection to itself.
///
/// Row streams hold their guard until they end or are dropped. A batch or write
/// waits until open row streams from this store end; do not start a batch or a
/// write on the same task while you hold a row stream from this store (it would
/// wait for itself). The raw `QueryStore::query` stream holds the read guard
/// when the SQL is a single `SELECT` or `EXPLAIN`, and the write guard for any
/// other statement (raw SQL can write, for example `INSERT ... RETURNING`).
///
/// The lock prefers writers, so a nested call on the same task can wait for a
/// writer that waits for your stream. Every wait ends after `lock_timeout`
/// (default 30 s, see `with_lock_timeout`) with `StorageError::LockTimeout`
/// naming the read or write side, so the mistake fails instead of hanging.
#[derive(Clone)]
pub struct TursoStorage {
    conn: Arc<turso::Connection>,
    gate: ConnGate,
    encryption_key: Option<EncryptionKey>,
}

impl TursoStorage {
    /// An in-memory database with the full schema migrated (`kv_store`,
    /// `documents`, …): nothing is persisted, so it suits tests and throwaway
    /// sessions. Use [`TursoStorage::new`] with a file path for a store that
    /// survives restarts.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database can't be opened or migrated.
    pub async fn in_memory() -> StorageResult<Self> {
        let storage = Self::new(":memory:").await?;
        storage.init_schema().await?;
        Ok(storage)
    }

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
            gate: ConnGate::new(),
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
        MigrationRunner::new(MIGRATIONS).run_async(self).await?;
        Ok(())
    }

    /// Run database migrations.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if migration execution fails.
    pub async fn migrate(&self, migrations: &[(&str, &str)]) -> StorageResult<()> {
        let _guard = self.write().await?;
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
        column_count: usize,
        column_names: &[String],
    ) -> StorageResult<SqlRow> {
        let mut columns = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let name = column_names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("col{i}"));
            let value = Self::turso_value_to_data_value(row.get_value(i)?);
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

    /// Set how long a call waits for the connection lock before it fails with
    /// [`StorageError::LockTimeout`]. Default: 30 s, the default acquire
    /// timeout of the `sqlx` pool. A task that holds a row stream and calls
    /// another operation on the same store waits for itself; this limit turns
    /// that mistake into an error instead of a hang. Call it before the first
    /// operation; clones made afterwards share the new limit.
    #[must_use]
    pub fn with_lock_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.gate = self.gate.with_timeout(timeout);
        self
    }

    // ========================================================================
    // Connection gate
    // ========================================================================

    /// Shared guard for an operation that only reads. Waits while a write or a
    /// batch is open. If an earlier rollback failed, the connection is checked
    /// first.
    async fn read(&self) -> StorageResult<ReadGuard> {
        loop {
            let guard = self.gate.read().await?;
            if !self.gate.needs_check() {
                return Ok(guard);
            }
            drop(guard);
            let _writer = self.write().await?;
        }
    }

    /// Exclusive guard for a write, a raw statement or a batch. If an earlier
    /// rollback failed, the connection is checked first.
    async fn write(&self) -> StorageResult<WriteGuard> {
        let guard = self.gate.write().await?;
        if self.gate.needs_check() {
            // Backend check: `turso::Connection::is_autocommit`. A connection
            // that is not in autocommit mode is still inside a transaction.
            let in_transaction = !self
                .conn
                .is_autocommit()
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            if in_transaction {
                self.conn.execute_batch("ROLLBACK").await.map_err(|e| {
                    StorageError::Backend(format!(
                        "connection is inside a transaction and rollback failed: {e}"
                    ))
                })?;
            }
            self.gate.clear_needs_check();
        }
        Ok(guard)
    }

    /// Roll back after `cause` failed a batch. The caller holds the write
    /// guard. Returns the error for the caller: the statement error, with the
    /// rollback error attached when the rollback failed too. A failed rollback
    /// marks the store so the next operation checks for an open transaction.
    async fn rollback_after(&self, cause: String) -> StorageError {
        let rollback = if self.gate.take_injected_rollback_failure() {
            Err("injected rollback failure".to_string())
        } else {
            self.conn
                .execute_batch("ROLLBACK")
                .await
                .map_err(|e| e.to_string())
        };
        match rollback {
            Ok(()) => StorageError::Backend(cause),
            Err(rollback_error) => {
                self.gate.mark_needs_check();
                StorageError::Backend(format!("{cause}; rollback also failed: {rollback_error}"))
            }
        }
    }

    /// Test hook: make the next batch rollback fail without running it, so the
    /// transaction stays open. Not part of the stable API.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn inject_rollback_failure(&self) {
        self.gate.inject_rollback_failure();
    }

    // ========================================================================
    // Async-First internal methods (source of truth for all operations)
    // ========================================================================

    /// Async KV get: select → decrypt → deserialize.
    async fn get_async_internal<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        let _guard = self.read().await?;
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
        let _guard = self.write().await?;
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
        let _guard = self.write().await?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Async KV take: one `DELETE ... RETURNING`, so only one caller gets the row.
    async fn take_async_internal<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        let _guard = self.write().await?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        let opt = conn
            .prepare("DELETE FROM kv_store WHERE key = ? RETURNING value")
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
                let json_str = self
                    .maybe_decrypt(&stored_value)
                    .map_err(|e| StorageError::Encryption(e.to_string()))?;
                let value: V = serde_json::from_str(&json_str)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Async KV exists.
    async fn exists_async_internal(&self, key: &str) -> StorageResult<bool> {
        let _guard = self.read().await?;
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
        let _guard = self.write().await?;
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
        let _guard = self.read().await?;
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
                Ok(Some(STANDARD.decode(&encoded).map_err(|e| {
                    StorageError::Backend(format!("Base64 decode failed: {e}"))
                })?))
            }
            None => Ok(None),
        }
    }

    /// Async blob delete.
    async fn delete_blob_async_internal(&self, key: &str) -> StorageResult<()> {
        let _guard = self.write().await?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute("DELETE FROM blobs WHERE key = ?", [key])
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Async blob exists.
    async fn blob_exists_async_internal(&self, key: &str) -> StorageResult<bool> {
        let _guard = self.read().await?;
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

    /// Creates a lazy async stream that yields SQL rows one at a time.
    ///
    /// NOTE: This is a **sync function** returning a stream — it does NOT block.
    /// The `.await` calls inside `try_stream!` are lazy: they only execute when
    /// the stream is polled. This function just constructs and returns the
    /// generator state machine.
    ///
    /// `query` wraps it in an `AsyncQueryStream` that the caller polls.
    fn query_rows_stream<G: Send + 'static>(
        conn: Arc<turso::Connection>,
        guard: G,
        sql: String,
        params: Vec<turso::Value>,
    ) -> impl AsyncStream<Item = StorageResult<SqlRow>> + Send {
        try_stream! {
            let _guard = guard;
            let mut stmt = conn.prepare(&sql).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt.query(params).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            // Column names live on the result set (Rows), not individual Row values.
            let column_names = rows.column_names();

            while let Some(row) = rows.next().await
                .map_err(|e| StorageError::Backend(e.to_string()))?
            {
                yield Self::turso_row_to_sql_row(
                    &row,
                    row.column_count(),
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
    /// `list_keys` wraps it in an `AsyncListStream`.
    fn list_keys_stream<G: Send + 'static>(
        conn: Arc<turso::Connection>,
        guard: G,
        sql: String,
        params: Vec<turso::Value>,
    ) -> impl AsyncStream<Item = StorageResult<String>> + Send {
        try_stream! {
            let _guard = guard;
            let mut stmt = conn.prepare(&sql).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let mut rows = stmt.query(params).await
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
    async fn execute_async_internal(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let _guard = self.write().await?;
        let turso_params = Self::to_turso_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);
        conn.execute(&sql, turso_params)
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))
    }

    /// Async `execute_batch`.
    async fn execute_batch_async_internal(&self, sql: &str) -> StorageResult<()> {
        let _guard = self.write().await?;
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
impl KeyValueStore for TursoStorage {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        self.get_async_internal::<V>(key).await
    }

    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        self.set_async_internal(key, value).await
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.delete_async_internal(key).await
    }

    fn take_is_atomic(&self) -> bool {
        true
    }

    async fn take<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        self.take_async_internal::<V>(key).await
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.exists_async_internal(key).await
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        // Without a prefix the query has no placeholder, so it binds nothing
        // (binding one gave "bind index 1 is out of bounds").
        let (sql, params) = match prefix {
            Some(p) => (
                "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key",
                Self::to_turso_params(&[DataValue::Text(format!("{p}%"))]),
            ),
            None => ("SELECT key FROM kv_store ORDER BY key", Vec::new()),
        };
        let guard = self.read().await?;
        let conn = Arc::clone(&self.conn);
        let stream = Self::list_keys_stream(conn, guard, sql.to_string(), params);
        Ok(AsyncListStream::new(stream))
    }
}

#[async_trait::async_trait]
impl QueryStore for TursoStorage {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        // A statement that can only read shares the connection. Anything else
        // (raw SQL can write, e.g. `INSERT ... RETURNING`) holds the write
        // guard until the stream ends or is dropped.
        let guard: Box<dyn Send> = if is_read_only_sql(sql) {
            Box::new(self.read().await?)
        } else {
            Box::new(self.write().await?)
        };
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        let params = Self::to_turso_params(params);
        let stream = Self::query_rows_stream(conn, guard, sql, params);
        Ok(AsyncQueryStream::new(stream))
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.execute_async_internal(sql, params).await
    }

    async fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        self.execute_batch_async_internal(sql).await
    }

    async fn execute_batch_statements(
        &self,
        statements: &[(String, Vec<DataValue>)],
    ) -> StorageResult<()> {
        // The write guard covers BEGIN .. COMMIT/ROLLBACK, so no other task
        // can run a statement inside this transaction.
        let _guard = self.write().await?;
        let conn = Arc::clone(&self.conn);
        conn.execute_batch("BEGIN")
            .await
            .map_err(|e| StorageError::Backend(e.to_string()))?;
        for (sql, params) in statements {
            let params = Self::to_turso_params(params);
            if let Err(e) = conn.execute(sql, params).await {
                return Err(self.rollback_after(e.to_string()).await);
            }
        }
        if let Err(e) = conn.execute_batch("COMMIT").await {
            return Err(self.rollback_after(e.to_string()).await);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl RateLimiterStore for TursoStorage {
    async fn check_rate_limit(
        &self,
        clock: &dyn Clock,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        rate_limit::sql_check(self, clock, key, max_count, window_seconds).await
    }

    async fn record_rate_limit(
        &self,
        clock: &dyn Clock,
        key: &str,
        window_seconds: u64,
    ) -> StorageResult<u32> {
        rate_limit::sql_record(self, clock, key, window_seconds).await
    }

    async fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        rate_limit::sql_reset(self, key).await
    }
}

#[async_trait::async_trait]
impl BlobStore for TursoStorage {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_blob_async_internal(key, data).await
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_blob_async_internal(key).await
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        self.delete_blob_async_internal(key).await
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        self.blob_exists_async_internal(key).await
    }
}
