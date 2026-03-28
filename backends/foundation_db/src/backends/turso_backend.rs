//! Turso storage backend implementation.
//!
//! Uses the Turso crate with async APIs wrapped via Valtron's
//! `from_future` + `execute` pattern to provide Valtron-native integration.
//! Multi-value operations return `StorageItemStream` for lazy iteration.

use crate::backends::async_utils::{exec_future, schedule_future};
use crate::errors::StorageResult;
use foundation_core::valtron::{Stream, StreamIteratorExt};
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

    /// Convert `turso::Row` to crate-owned [`SqlRow`].
    fn turso_row_to_sql_row(row: &turso::Row) -> StorageResult<SqlRow> {
        let column_count = row.column_count();
        let mut columns = Vec::with_capacity(column_count);

        for i in 0..column_count {
            let name = format!("col{i}");
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
}

impl KeyValueStore for TursoStorage {
    fn get<V: DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, Option<V>>> {
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
        }, |e| {
            tracing::error!("get error: {e}");
            None
        })?;

        Ok(Box::new(stream.map_done(|opt_json| {
            opt_json.and_then(|json_str| serde_json::from_str::<V>(&json_str).ok())
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
        }, |_| 0u64)?;

        Ok(Box::new(stream.map_done(|_rows| ())))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
                .await
        }, |_| 0u64)?;

        Ok(Box::new(stream.map_done(|_rows| ())))
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
        }, |e| {
            tracing::error!("exists error: {e}");
            false
        })?;

        Ok(Box::new(stream))
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
                stmt.query([turso::Value::Null; 0]).await?
            } else {
                stmt.query([param]).await?
            };

            let mut keys = Vec::new();
            while let Some(row) = rows.next().await? {
                let key: String = row.get(0)?;
                keys.push(key);
            }
            Ok::<_, turso::Error>(keys)
        }, Vec::new)?;

        Ok(Box::new(stream.flat_map_next(|keys| keys)))
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

        let rows_stream = schedule_future(async move {
            let mut stmt = conn.prepare(&sql).await?;
            let mut rows = stmt.query(turso_params).await?;

            let mut rows_data = Vec::new();
            while let Some(row) = rows.next().await? {
                let sql_row = Self::turso_row_to_sql_row(&row)
                    .map_err(|e| turso::Error::ConversionFailure(e.to_string()))?;
                rows_data.push(sql_row);
            }
            Ok::<_, turso::Error>(rows_data)
        }, Vec::new)?;

        Ok(Box::new(rows_stream.flat_map_next(|rows| rows)))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        let turso_params = Self::to_turso_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move { conn.execute(&sql, turso_params).await }, |_| 0)?;

        Ok(Box::new(raw_stream.map_done(|rows| rows as u64)))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move { conn.execute_batch(&sql).await }, |_| ())?;
        Ok(Box::new(stream))
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
        }, |e| {
            tracing::error!("check_rate_limit error: {e}");
            false
        })?;

        Ok(Box::new(stream))
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
        schedule_future(async move {
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
        }, |e| {
            tracing::error!("record_rate_limit error: {e}");
            0
        })?;

        Ok(Box::new(stream))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let stream = schedule_future(async move {
            conn.execute("DELETE FROM rate_limits WHERE key = ?", [key])
                .await
        }, |_| ())?;

        Ok(Box::new(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_core::valtron::{collect_one, collect_result};
    use tempfile::TempDir;

    /// Initialize the Valtron executor for tests.
    fn init_valtron() {
        foundation_core::valtron::single::initialize_pool(42);
    }

    #[test]
    fn test_turso_storage_basic() {
        init_valtron();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = TursoStorage::new(url).unwrap();
        storage.init_schema().unwrap();

        collect_one(storage.set("test_key", "test_value").unwrap()).unwrap();

        let value: Option<String> = collect_one(storage.get("test_key").unwrap()).unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        assert!(collect_one(storage.exists("test_key").unwrap()).unwrap());
        assert!(!collect_one(storage.exists("nonexistent").unwrap()).unwrap());

        collect_one(storage.delete("test_key").unwrap()).unwrap();
        assert!(!collect_one(storage.exists("test_key").unwrap()).unwrap());
    }

    #[test]
    fn test_turso_storage_list_keys() {
        init_valtron();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = TursoStorage::new(url).unwrap();
        storage.init_schema().unwrap();

        collect_one(storage.set("prefix:key1", "value1").unwrap()).unwrap();
        collect_one(storage.set("prefix:key2", "value2").unwrap()).unwrap();
        collect_one(storage.set("other:key3", "value3").unwrap()).unwrap();

        let keys = collect_result(storage.list_keys(None).unwrap());
        assert_eq!(keys.len(), 3);

        let keys = collect_result(storage.list_keys(Some("prefix:")).unwrap());
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_turso_storage_migrations() {
        init_valtron();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = TursoStorage::new(url).unwrap();

        let migrations = &[
            (
                "001_create_users",
                "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL)",
            ),
            (
                "002_create_sessions",
                "CREATE TABLE sessions (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)",
            ),
        ];

        storage.migrate(migrations).unwrap();

        let users_exist = !collect_result(
            storage
                .query(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='users'",
                    &[],
                )
                .unwrap(),
        )
        .is_empty();

        let sessions_exist = !collect_result(
            storage
                .query(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions'",
                    &[],
                )
                .unwrap(),
        )
        .is_empty();

        let migrations_exist = !collect_result(
            storage
                .query(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations'",
                    &[],
                )
                .unwrap(),
        )
        .is_empty();

        assert!(users_exist, "users table should be accessible");
        assert!(sessions_exist, "sessions table should be accessible");
        assert!(migrations_exist, "_migrations table should be accessible");
    }
}
