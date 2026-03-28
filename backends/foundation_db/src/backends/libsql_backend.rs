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
    fn get<V: DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, Option<V>>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future::<_, _, _, _>(async move {
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

        let mapped = raw_stream
            .map_done(|opt_json| {
                opt_json.and_then(|json_str| serde_json::from_str::<V>(&json_str).ok())
            })
            .map_pending(|_| ());

        Ok(Box::new(mapped))
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let serialized = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move {
            conn.execute(
                "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
                [key.clone(), serialized.clone(), serialized],
            )
            .await
        })?;

        let mapped = raw_stream.map_done(|_rows| ()).map_pending(|_| ());

        Ok(Box::new(mapped))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move {
            conn.execute("DELETE FROM kv_store WHERE key = ?", [key])
                .await
        })?;

        let mapped = raw_stream.map_done(|_rows| ()).map_pending(|_| ());

        Ok(Box::new(mapped))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        schedule_future::<_, libsql::Error, _>(async move {
            let mut stmt = conn
                .prepare("SELECT 1 FROM kv_store WHERE key = ? LIMIT 1")
                .await?;
            let mut rows = stmt.query([key]).await?;
            Ok(rows.next().await?.is_some())
        })
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

        let keys_stream = schedule_future(async move {
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

        // Flatten Vec<String> stream into individual String items
        let mapped = keys_stream.flat_map(|s| match s {
            Stream::Next(keys) => {
                let items: Vec<Stream<String, ()>> = keys.into_iter().map(Stream::Next).collect();
                items.into_iter()
            }
            Stream::Pending(p) => vec![Stream::Pending(p)].into_iter(),
            Stream::Init => vec![Stream::Init].into_iter(),
            Stream::Ignore => vec![Stream::Ignore].into_iter(),
            Stream::Delayed(d) => vec![Stream::Delayed(d)].into_iter(),
        });

        Ok(Box::new(mapped))
    }
}

impl QueryStore for LibsqlStorage {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let rows_stream = schedule_future::<_, _, _, _>(async move {
            let mut stmt = conn.prepare(&sql).await?;
            let mut rows = stmt.query(libsql_params).await?;

            let column_count = rows.column_count();
            let mut rows_data = Vec::new();
            while let Some(row) = rows.next().await? {
                let sql_row = Self::libsql_row_to_sql_row(&row, column_count)
                    .map_err(|e| libsql::Error::ToSqlConversionFailure(Box::new(e)))?;
                rows_data.push(sql_row);
            }
            Ok::<_, libsql::Error>(rows_data)
        })?;

        let mapped = rows_stream.flat_map_next(|rows| rows);

        Ok(Box::new(mapped))
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        let libsql_params = Self::to_libsql_params(params);
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move { conn.execute(&sql, libsql_params).await })?;

        let mapped = raw_stream.map_done(|rows| rows as u64).map_pending(|_| ());

        Ok(Box::new(mapped))
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let sql = sql.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move { conn.execute_batch(&sql).await })?;

        let mapped = raw_stream.map_done(|_rows| ()).map_pending(|_| ());

        Ok(Box::new(mapped))
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

        schedule_future::<_, _, _, _>(async move {
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
        })
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .cast_signed();

        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        schedule_future::<_, _, _, _>(async move {
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
        })
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let key = key.to_string();
        let conn = Arc::clone(&self.conn);

        let raw_stream = schedule_future(async move {
            conn.execute("DELETE FROM rate_limits WHERE key = ?", [key])
                .await
        })?;

        let mapped = raw_stream.map_done(|_rows| ()).map_pending(|_| ());

        Ok(Box::new(mapped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Initialize the Valtron executor for tests.
    fn init_valtron() {
        foundation_core::valtron::single::initialize_pool(42);
    }

    #[test]
    fn test_libsql_storage_basic() {
        init_valtron();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = LibsqlStorage::new(url).unwrap();
        storage.init_schema().unwrap();

        storage.set("test_key", "test_value").unwrap();

        let value: String = storage.get("test_key").unwrap().unwrap();
        assert_eq!(value, "test_value");

        assert!(storage.exists("test_key").unwrap());
        assert!(!storage.exists("nonexistent").unwrap());

        storage.delete("test_key").unwrap();
        assert!(!storage.exists("test_key").unwrap());
    }

    #[test]
    fn test_libsql_storage_list_keys() {
        init_valtron();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = LibsqlStorage::new(url).unwrap();
        storage.init_schema().unwrap();

        storage.set("prefix:key1", "value1").unwrap();
        storage.set("prefix:key2", "value2").unwrap();
        storage.set("other:key3", "value3").unwrap();

        let keys: Vec<String> = storage
            .list_keys(None)
            .unwrap()
            .filter_map(|s| match s {
                Stream::Next(key) => Some(key),
                _ => None,
            })
            .collect();
        assert_eq!(keys.len(), 3);

        let keys: Vec<String> = storage
            .list_keys(Some("prefix:"))
            .unwrap()
            .filter_map(|s| match s {
                Stream::Next(key) => Some(key),
                _ => None,
            })
            .collect();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_libsql_storage_migrations() {
        init_valtron();
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = LibsqlStorage::new(url).unwrap();

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

        let users_exist = storage
            .query(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='users'",
                &[],
            )
            .unwrap()
            .next()
            .is_some();

        let sessions_exist = storage
            .query(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions'",
                &[],
            )
            .unwrap()
            .next()
            .is_some();

        let migrations_exist = storage
            .query(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations'",
                &[],
            )
            .unwrap()
            .next()
            .is_some();

        assert!(users_exist, "users table should be accessible");
        assert!(sessions_exist, "sessions table should be accessible");
        assert!(migrations_exist, "_migrations table should be accessible");
    }
}
