//! Turso storage backend implementation.
//!
//! Uses the Turso crate with async APIs wrapped via Valtron's
//! `from_future` + `execute` pattern to provide Valtron-native integration.
//! Multi-value operations return `StorageItemStream` for lazy iteration.

use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageResult;
use foundation_core::valtron::{Stream, StreamIteratorExt, ThreadedFuture, ThreadedValue};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use turso::Builder;

use crate::errors::StorageError;
use crate::storage_provider::{
    DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// Turso storage backend.
pub struct TursoStorage {
    conn: Arc<turso::Connection>,
}

impl TursoStorage {
    /// Create a new Turso storage connection.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub fn new(url: &str) -> StorageResult<Self> {
        let url = url.to_string();
        let db: turso::Database =
            exec_future(async move { Builder::new_local(&url).build().await })?;
        let conn = db
            .connect()
            .map_err(|e| StorageError::Backend(format!("Connection failed: {e}")))?;
        Ok(Self {
            conn: Arc::new(conn),
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
}

impl KeyValueStore for TursoStorage {
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT value FROM kv_store WHERE key = ?")
                .await?;
            let mut rows = stmt.query([key]).await?;
            match rows.next().await? {
                Some(row) => {
                    let value: String = row.get(0)?;
                    Ok::<_, turso::Error>(Some(value))
                }
                None => Ok::<_, turso::Error>(None),
            }
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(opt) => ShortCircuit::Continue(Stream::Next(Ok(opt))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream.map_done(|opt_result| {
            // opt_result: Result<Option<String>, StorageError>
            // We want: Result<Option<V>, StorageError>
            match opt_result {
                Ok(Some(json_str)) => serde_json::from_str::<V>(&json_str)
                    .map(Some)
                    .map_err(|e| StorageError::Serialization(e.to_string())),
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            }
        })))
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let serialized = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute(
                "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
                [key.clone(), serialized.clone(), serialized],
            )
            .await
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        // Then map_done to transform Ok(u64) -> Ok(())
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
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
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
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
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(val) => ShortCircuit::Continue(Stream::Next(Ok(val))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        use crate::rows_stream::RowsIterator;

        let (sql, param): (&str, String) = match prefix {
            Some(p) => (
                "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key",
                format!("{p}%"),
            ),
            None => ("SELECT key FROM kv_store ORDER BY key", String::new()),
        };

        let turso_params = Self::to_turso_params(&[DataValue::Text(param.clone())]);
        let conn = Arc::clone(&self.conn);

        // Use ThreadedFuture for streaming query results
        let threaded = ThreadedFuture::new(move || async move {
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

            Ok::<_, StorageError>(RowsIterator::new(rows))
        });

        let receiver = threaded.execute()
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // RowsIterator yields Result<SqlRow, StorageError>
        // Convert ThreadedValue to Stream
        let block_iter = receiver.into_recv_iter();
        let stream = block_iter.map(|threaded_value| match threaded_value {
            ThreadedValue::Value(row_result) => match row_result {
                Ok(row) => match row.get::<String>(0) {
                    Ok(key) => Stream::Next(Ok(key)),
                    Err(e) => Stream::Next(Err(StorageError::SqlConversion(e.to_string()))),
                },
                Err(e) => Stream::Next(Err(e)),
            },
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
        use crate::rows_stream::RowsIterator;
        use foundation_core::valtron::ThreadedFuture;

        let turso_params = Self::to_turso_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        // Use ThreadedFuture to spawn worker thread that owns !Send turso::Rows
        // RowsIterator already converts turso::Error to StorageError internally
        let threaded = ThreadedFuture::new(move || async move {
            let mut stmt = conn
                .prepare(&sql)
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt
                .query(turso_params)
                .await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(RowsIterator::new(rows))
        });

        let receiver = threaded.execute()
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        // RowsIterator yields Result<SqlRow, StorageError>
        // Convert ThreadedValue to Stream
        let block_iter = receiver.into_recv_iter();
        let stream = block_iter.map(|threaded_value| match threaded_value {
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
        let circuit_stream = raw_stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move { conn.execute_batch(&sql).await })?;

        // Use map_circuit to yield Ok(()) or Err(e)
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(()) => ShortCircuit::Continue(Stream::Next(Ok(()))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
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
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(val) => ShortCircuit::Continue(Stream::Next(Ok(val))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
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
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(val) => ShortCircuit::Continue(Stream::Next(Ok(val))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                        StorageError::Backend(e.to_string()),
                    ))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
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
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(Ok(rows)) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                Stream::Next(Err(e)) => ShortCircuit::ReturnAndStop(Stream::Next(Err(
                    StorageError::Backend(e.to_string()),
                ))),
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(
            circuit_stream.map_done(|result| result.map(|_| ())),
        ))
    }
}
