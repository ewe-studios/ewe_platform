//! libsql storage backend implementation.
//!
//! Uses the libsql crate with async APIs wrapped via Valtron's
//! `from_future` + `execute` pattern to provide Valtron-native integration.
//! Multi-value operations return `StorageItemStream` for lazy iteration.

use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageResult;
use foundation_core::valtron::{Stream, StreamIteratorExt};
use libsql::Builder;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use crate::errors::StorageError;
use crate::storage_provider::{
    DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// libsql storage backend.
pub struct LibsqlStorage {
    conn: Arc<libsql::Connection>,
}

impl LibsqlStorage {
    /// Create a new libsql storage connection.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the database connection fails.
    pub fn new(url: &str) -> StorageResult<Self> {
        let url = url.to_string();
        let db = exec_future(async move { Builder::new_local(&url).build().await })?;
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
                    Ok::<_, libsql::Error>(false)
                } else {
                    conn.execute_batch(&sql).await?;
                    conn.execute(
                        "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                        [id.clone(), id],
                    )
                    .await?;
                    Ok::<_, libsql::Error>(true)
                }
            })?;
        }

        Ok(())
    }

    /// Convert crate-owned [`DataValue`] slice to `libsql::Value` Vec.
    fn to_libsql_params(params: &[DataValue]) -> Vec<libsql::Value> {
        params.iter().map(Self::data_value_to_libsql).collect()
    }

    /// Convert a single [`DataValue`] to `libsql::Value`.
    fn data_value_to_libsql(value: &DataValue) -> libsql::Value {
        match value {
            DataValue::Null => libsql::Value::Null,
            DataValue::Integer(i) => libsql::Value::Integer(*i),
            DataValue::Real(r) => libsql::Value::Real(*r),
            DataValue::Text(s) => libsql::Value::Text(s.clone()),
            DataValue::Blob(b) => libsql::Value::Blob(b.clone()),
        }
    }

    /// Convert `libsql::Row` to crate-owned [`SqlRow`].
    fn libsql_row_to_sql_row(row: &libsql::Row, column_count: i32) -> StorageResult<SqlRow> {
        let mut columns = Vec::with_capacity(column_count.unsigned_abs() as usize);

        for i in 0..column_count {
            let name = format!("col{i}");
            let value = Self::libsql_value_to_data_value(row.get_value(i)?);
            columns.push((name, value));
        }

        Ok(SqlRow::new(columns))
    }

    /// Convert `libsql::Value` to crate-owned [`DataValue`].
    fn libsql_value_to_data_value(value: libsql::Value) -> DataValue {
        match value {
            libsql::Value::Null => DataValue::Null,
            libsql::Value::Integer(i) => DataValue::Integer(i),
            libsql::Value::Real(r) => DataValue::Real(r),
            libsql::Value::Text(s) => DataValue::Text(s),
            libsql::Value::Blob(b) => DataValue::Blob(b),
        }
    }
}

impl KeyValueStore for LibsqlStorage {
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
                    Ok::<_, libsql::Error>(Some(value))
                }
                None => Ok::<_, libsql::Error>(None),
            }
        })?;

        // Use map_circuit to short-circuit on error, yielding error in stream
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(opt) => ShortCircuit::Continue(Stream::Next(Ok(opt))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream.map_done(|opt_result| {
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

        // Use map_circuit to yield errors in stream, then map_done to transform u64 -> ()
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream.map_done(|result| result.map(|_rows| ()))))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
                .await
        })?;

        // Use map_circuit to yield errors in stream, then map_done to transform u64 -> ()
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream.map_done(|result| result.map(|_rows| ()))))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn
                .prepare("SELECT 1 FROM kv_store WHERE key = ? LIMIT 1")
                .await?;
            let mut rows = stmt.query([key]).await?;
            Ok(rows.next().await?.is_some())
        })?;

        // Use map_circuit to yield errors in stream
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(b) => ShortCircuit::Continue(Stream::Next(Ok(b))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
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

        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            let mut stmt = conn.prepare(sql).await?;
            let mut rows = if param.is_empty() {
                stmt.query([libsql::Value::Null; 0]).await?
            } else {
                stmt.query([param]).await?
            };

            let mut keys = Vec::new();
            while let Some(row) = rows.next().await? {
                if let Ok(key) = row.get::<String>(0) {
                    keys.push(key);
                }
            }
            Ok::<_, libsql::Error>(keys)
        })?;

        // Use map_circuit to yield errors, then flat_map_next to expand Vec into iterator
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(keys) => ShortCircuit::Continue(Stream::Next(Ok(keys))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream.flat_map_next(|keys_result| match keys_result {
            Ok(keys) => keys.into_iter().map(Ok).collect(),
            Err(e) => vec![Err(e)],
        })))
    }
}

impl QueryStore for LibsqlStorage {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        use crate::rows_stream::LibsqlRowsIterator;
        use foundation_core::valtron::{ThreadedFuture, ThreadedValue};
        use foundation_core::synca::mpp::Stream;

        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        // Use ThreadedFuture to spawn worker thread that owns !Send libsql::Rows
        let threaded = ThreadedFuture::new(move || async move {
            let mut stmt = conn.prepare(&sql).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            let rows = stmt.query(libsql_params).await
                .map_err(|e| StorageError::Backend(e.to_string()))?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows))
        });

        // execute() returns Result<impl Iterator, Error> in multi mode,
        // or impl Iterator directly in single-threaded mode
        #[cfg(feature = "multi")]
        let iter = threaded
            .execute()
            .map_err(|e| StorageError::Backend(e.to_string()))?;

        #[cfg(not(feature = "multi"))]
        let iter = threaded.execute();

        // Convert ThreadedValue to Stream
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
        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move { conn.execute(&sql, libsql_params).await })?;

        // Use map_circuit to yield errors in stream
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(rows) => ShortCircuit::Continue(Stream::Next(Ok(rows as u64))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
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

        // Use map_circuit to yield errors in stream, then map_done to transform Result<(), libsql::Error> -> Result<(), StorageError>
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(_) => ShortCircuit::Continue(Stream::Next(Ok(()))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream))
    }
}

impl RateLimiterStore for LibsqlStorage {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
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
                        Ok::<_, libsql::Error>(true)
                    } else {
                        Ok::<_, libsql::Error>(count < i64::from(max_count))
                    }
                }
                None => Ok::<_, libsql::Error>(true),
            }
        })?;

        // Use map_circuit to yield errors in stream
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(b) => ShortCircuit::Continue(Stream::Next(Ok(b))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
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
                    Ok::<_, libsql::Error>(
                        u32::try_from(count)
                            .map_err(|e| libsql::Error::ToSqlConversionFailure(Box::new(e)))?,
                    )
                }
                None => Ok::<_, libsql::Error>(1),
            }
        })?;

        // Use map_circuit to yield errors in stream
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(count) => ShortCircuit::Continue(Stream::Next(Ok(count))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
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

        // Use map_circuit to yield errors in stream, then map_done to transform
        let circuit_stream = stream.map_circuit(|stream_item| {
            use foundation_core::valtron::ShortCircuit;
            match stream_item {
                Stream::Next(result) => match result {
                    Ok(_) => ShortCircuit::Continue(Stream::Next(Ok(()))),
                    Err(e) => ShortCircuit::ReturnAndStop(Stream::Next(Err(StorageError::Backend(e.to_string())))),
                },
                _ => ShortCircuit::Continue(Stream::Ignore),
            }
        });

        Ok(Box::new(circuit_stream))
    }
}
