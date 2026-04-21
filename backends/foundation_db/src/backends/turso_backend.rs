//! Turso storage backend implementation.
//!
//! Uses the Turso crate with async APIs wrapped via Valtron's
//! `from_future` + `execute` pattern to provide Valtron-native integration.
//! Multi-value operations return `StorageItemStream` for lazy iteration.

use crate::backends::async_utils::{exec_future, schedule_future};
use crate::crypto::{decrypt, encrypt, EncryptionKey};
use crate::errors::StorageResult;
use base64::{engine::general_purpose::STANDARD, Engine};
use foundation_core::valtron::{
    run_future_iter, ShortCircuit, Stream, StreamIteratorExt, ThreadedValue,
};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use turso::Builder;

use crate::errors::StorageError;
use crate::rows_stream::RowsIterator;
use crate::storage_provider::{
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
        let schema_sql = r"
            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            );

            CREATE INDEX IF NOT EXISTS idx_kv_store_key ON kv_store(key);

            CREATE TABLE IF NOT EXISTS _migrations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            );
        ";

        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(schema_sql).await })?;
        Ok(())
    }

    /// Run database migrations.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if migration execution fails.
    pub fn migrate(&self, migrations: &[(&str, &str)]) -> StorageResult<()> {
        // Create migrations table
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

            // Check if migration exists and apply if not - all in one async block
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
    ///
    /// # Errors
    ///
    /// Returns an error if encryption fails.
    fn maybe_encrypt(&self, json_str: &str) -> StorageResult<String> {
        match &self.encryption_key {
            Some(key) => {
                let encrypted = encrypt(key, json_str.as_bytes())?;
                // Encode as base64 for safe storage in TEXT column
                Ok(STANDARD.encode(&encrypted))
            }
            None => Ok(json_str.to_string()),
        }
    }

    /// Decrypt a value if encryption is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if decryption fails.
    fn maybe_decrypt(&self, stored_value: &str) -> StorageResult<String> {
        match &self.encryption_key {
            Some(key) => {
                // Decode from base64
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
}

impl KeyValueStore for TursoStorage {
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);
        let storage = self.clone();

        // First, get the encrypted value from the database
        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT value FROM kv_store WHERE key = ?")
                .await?;
            let mut rows = stmt.query([key]).await?;
            match rows.next().await? {
                Some(row) => {
                    let stored_value: String = row.get(0)?;
                    Ok::<_, turso::Error>(Some(stored_value))
                }
                None => Ok::<_, turso::Error>(None),
            }
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(opt) => ShortCircuit::Continue(Stream::Next(Ok(opt))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        // Now decrypt and deserialize in map_done
        Ok(Box::new(circuit_stream.map_done(
            move |opt_result: Result<Option<String>, StorageError>| {
                match opt_result {
                    Ok(Some(stored_value)) => {
                        // Decrypt the value
                        let json_str = storage
                            .maybe_decrypt(&stored_value)
                            .map_err(|e| StorageError::Encryption(e.to_string()))?;
                        // Deserialize
                        let value: V = serde_json::from_str(&json_str)
                            .map_err(|e| StorageError::Serialization(e.to_string()))?;
                        Ok(Some(value))
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(e),
                }
            },
        )))
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let serialized = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        // Encrypt if encryption is enabled
        let stored_value = self.maybe_encrypt(&serialized)?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute(
                "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
                [key.clone(), stored_value.clone(), stored_value],
            )
            .await
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        // Then map_done to transform Ok(u64) -> Ok(())
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(
            circuit_stream.map_done(|result| result.map(|_rows| ())),
        ))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
                .await
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(
            circuit_stream.map_done(|result| result.map(|_| ())),
        ))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT 1 FROM kv_store WHERE key = ? LIMIT 1")
                .await?;
            let mut rows = stmt.query([key]).await?;
            Ok::<_, turso::Error>(rows.next().await?.is_some())
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(val) => ShortCircuit::Continue(Stream::Next(Ok(val))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let (sql, param): (&str, String) = match prefix {
            Some(p) => (
                "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key",
                format!("{p}%"),
            ),
            None => ("SELECT key FROM kv_store ORDER BY key", String::new()),
        };

        let turso_params = Self::to_turso_params(&[DataValue::Text(param.clone())]);
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                let mut stmt = conn
                    .prepare(sql)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let rows = if param.is_empty() {
                    stmt.query([turso::Value::Null; 0]).await
                } else {
                    stmt.query(turso_params).await
                }
                .map_err(|e| StorageError::Backend(e.to_string()))?;

                Ok::<_, StorageError>(RowsIterator::new(rows, |row| {
                    row.get::<String>(0)
                        .map_err(|e| StorageError::SqlConversion(e.to_string()))
                }))
            },
            None,
            None,
        )
        .map_err(|e| StorageError::Backend(e.to_string()))?;

        let stream = iter.map(|threaded_value| match threaded_value {
            ThreadedValue::Value(result) => Stream::Next(result),
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
        let turso_params = Self::to_turso_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let iter = run_future_iter(
            move || async move {
                let mut stmt = conn
                    .prepare(&sql)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                let rows = stmt
                    .query(turso_params)
                    .await
                    .map_err(|e| StorageError::Backend(e.to_string()))?;
                Ok::<_, StorageError>(RowsIterator::new(rows, |row| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    Self::turso_row_to_sql_row(row, row.column_count() as i32)
                }))
            },
            None,
            None,
        )
        .map_err(|e| StorageError::Backend(e.to_string()))?;

        let stream = iter.map(|threaded_value| match threaded_value {
            ThreadedValue::Value(result) => Stream::Next(result),
        });

        Ok(Box::new(stream))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        let turso_params = Self::to_turso_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move { conn.execute(&sql, turso_params).await })?;

        // Use map_circuit to yield Ok(rows) or Err(e)
        let circuit_stream = raw_stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move { conn.execute_batch(&sql).await })?;

        // Use map_circuit to yield Ok(()) or Err(e)
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(()) => ShortCircuit::Continue(Stream::Next(Ok(()))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream))
    }
}

impl RateLimiterStore for TursoStorage {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
        // Ensure rate_limits table exists (one-shot init — exec_future is acceptable)
        let create_table = r"
            CREATE TABLE IF NOT EXISTS rate_limits (
                key TEXT PRIMARY KEY,
                count INTEGER NOT NULL,
                window_start INTEGER NOT NULL
            )
        ";
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(create_table).await })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_start = now - window_seconds;

        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT count, window_start FROM rate_limits WHERE key = ?")
                .await?;
            let mut rows = stmt.query([key]).await?;
            match rows.next().await? {
                Some(row) => {
                    let count: i64 = row.get(0)?;
                    let stored_window_start: i64 = row.get(1)?;

                    if stored_window_start.cast_unsigned() < window_start {
                        Ok::<_, turso::Error>(true)
                    } else {
                        Ok::<_, turso::Error>(count < i64::from(max_count))
                    }
                }
                None => Ok::<_, turso::Error>(true),
            }
        })?;

        // Use map_circuit to yield Ok(value) or Err(e)
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(val) => ShortCircuit::Continue(Stream::Next(Ok(val))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .cast_signed();

        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        // Combine insert + read into one async block for atomicity
        let stream = schedule_future(async move {
            conn.execute(
                "INSERT INTO rate_limits (key, count, window_start) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1, window_start = excluded.window_start",
                [key.clone(), now.to_string()],
            )
            .await?;

            let mut stmt = conn
                .prepare("SELECT count FROM rate_limits WHERE key = ?")
                .await?;
            let mut rows = stmt.query([key]).await?;
            match rows.next().await? {
                Some(row) => {
                    let count: i64 = row.get(0)?;
                    Ok::<_, turso::Error>(u32::try_from(count).map_err(|e| {
                        turso::Error::ConversionFailure(format!("Cannot convert count to u32: {e}"))
                    })?)
                }
                None => Ok::<_, turso::Error>(1),
            }
        })?;

        // Use map_circuit to yield Ok(value) or Err(e)
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(val) => ShortCircuit::Continue(Stream::Next(Ok(val))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute("DELETE FROM rate_limits WHERE key = ?", [key])
                .await
        })?;

        // Use map_circuit for error propagation, then map_done to transform u64 -> ()
        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(Ok(rows)) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
            Stream::Next(Err(e)) => {
                ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string()))))
            }
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(
            circuit_stream.map_done(|result| result.map(|_| ())),
        ))
    }
}

impl BlobStore for TursoStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        // Ensure blobs table exists
        let create_table = r"
            CREATE TABLE IF NOT EXISTS blobs (
                key TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                size INTEGER NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            )
        ";
        let conn = Arc::clone(&self.conn);
        exec_future(async move { conn.execute_batch(create_table).await })?;

        // Encode binary data as base64 for safe storage
        let encoded = STANDARD.encode(data);
        let key = key.to_string();
        #[allow(clippy::cast_possible_wrap)]
        let size = data.len() as i64;
        let size_str = size.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute(
                "INSERT INTO blobs (key, data, size, updated_at) VALUES (?, ?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET data = ?, size = ?, updated_at = strftime('%s', 'now') * 1000",
                [key.clone(), encoded.clone(), size_str.clone(), encoded, size_str],
            )
            .await
        })?;

        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(
            circuit_stream.map_done(|result| result.map(|_rows| ())),
        ))
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn.prepare("SELECT data FROM blobs WHERE key = ?").await?;
            let mut rows = stmt.query([key]).await?;
            match rows.next().await? {
                Some(row) => {
                    let encoded: String = row.get(0)?;
                    Ok::<_, turso::Error>(Some(encoded))
                }
                None => Ok::<_, turso::Error>(None),
            }
        })?;

        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(opt) => ShortCircuit::Continue(Stream::Next(Ok(opt))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream.map_done(|opt_result| {
            match opt_result {
                Ok(Some(encoded)) => STANDARD
                    .decode(&encoded)
                    .map(Some)
                    .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}"))),
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            }
        })))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute("DELETE FROM blobs WHERE key = ?", [key]).await
        })?;

        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(
            circuit_stream.map_done(|result| result.map(|_rows| ())),
        ))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT 1 FROM blobs WHERE key = ? LIMIT 1")
                .await?;
            let mut rows = stmt.query([key]).await?;
            Ok::<bool, turso::Error>(rows.next().await?.is_some())
        })?;

        let circuit_stream = stream.map_circuit(|stream_item| match stream_item {
            Stream::Next(result) => match result {
                Ok(b) => ShortCircuit::Continue(Stream::Next(Ok(b))),
                Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(
                    e.to_string(),
                )))),
            },
            _ => ShortCircuit::Continue(Stream::Ignore),
        });

        Ok(Box::new(circuit_stream))
    }
}
