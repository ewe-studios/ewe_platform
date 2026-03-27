//! Turso storage backend implementation.
//!
//! Uses the Turso crate with async APIs wrapped in blocking calls
//! via `futures_lite::future::block_on` to provide a synchronous interface.
//! Multi-value operations return `StorageItemStream` for lazy iteration.

use futures_lite::future::block_on;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use crate::errors::StorageResult;
use crate::storage_provider::{DataValue, KeyValueStore, QueryStore, SqlRow, StorageItemStream};
use turso::Builder;

/// Turso storage backend.
pub struct TursoStorage {
    conn: Arc<turso::Connection>,
}

impl TursoStorage {
    /// Create a new Turso storage connection.
    pub fn new(url: &str) -> StorageResult<Self> {
        // Turso uses Builder pattern: Builder::new_local(path).build()
        let db = block_on(async {
            Builder::new_local(url).build().await
        })?;
        let conn = db.connect()?;
        Ok(Self { conn: Arc::new(conn) })
    }

    /// Initialize the database schema.
    pub fn init_schema(&self) -> StorageResult<()> {
        let schema_sql = r#"
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
        "#;

        block_on(async { self.conn.execute_batch(schema_sql).await })?;
        Ok(())
    }

    /// Run database migrations.
    pub fn migrate(&self, migrations: &[(&str, &str)]) -> StorageResult<()> {
        // Create migrations table if it doesn't exist
        block_on(async {
            self.conn
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS _migrations (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL,
                        applied_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
                    )",
                )
                .await
        })?;

        for (id, sql) in migrations {
            // Check if migration already applied
            let mut stmt = block_on(async {
                self.conn
                    .prepare("SELECT 1 FROM _migrations WHERE id = ?")
                    .await
            })?;

            let mut rows = block_on(async {
                stmt.query([id.to_string()]).await
            })?;

            let exists = block_on(async { rows.next().await })?.is_some();

            if !exists {
                // Apply migration
                block_on(async { self.conn.execute_batch(sql).await })?;

                // Record migration
                block_on(async {
                    self.conn
                        .execute(
                            "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                            [id.to_string(), id.to_string()],
                        )
                        .await
                })?;
            }
        }

        Ok(())
    }

    /// Convert crate-owned DataValue slice to turso::Value Vec.
    fn to_turso_params(params: &[DataValue]) -> Vec<turso::Value> {
        params.iter().map(Self::data_value_to_turso).collect()
    }

    /// Convert a single DataValue to turso::Value.
    fn data_value_to_turso(value: &DataValue) -> turso::Value {
        match value {
            DataValue::Null => turso::Value::Null,
            DataValue::Integer(i) => turso::Value::Integer(*i),
            DataValue::Real(r) => turso::Value::Real(*r),
            DataValue::Text(s) => turso::Value::Text(s.clone()),
            DataValue::Blob(b) => turso::Value::Blob(b.clone()),
        }
    }

    /// Convert turso::Row to crate-owned SqlRow.
    fn turso_row_to_sql_row(row: &turso::Row) -> StorageResult<SqlRow> {
        let column_count = row.column_count();
        let mut columns = Vec::with_capacity(column_count);

        for i in 0..column_count {
            let name = format!("col{i}");
            let value = Self::turso_value_to_data_value(row.get_value(i)?)?;
            columns.push((name, value));
        }

        Ok(SqlRow::new(columns))
    }

    /// Convert turso::Value to crate-owned DataValue.
    fn turso_value_to_data_value(value: turso::Value) -> StorageResult<DataValue> {
        Ok(match value {
            turso::Value::Null => DataValue::Null,
            turso::Value::Integer(i) => DataValue::Integer(i),
            turso::Value::Real(r) => DataValue::Real(r),
            turso::Value::Text(s) => DataValue::Text(s),
            turso::Value::Blob(b) => DataValue::Blob(b),
        })
    }
}

impl KeyValueStore for TursoStorage {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let mut stmt = block_on(async {
            self.conn
                .prepare("SELECT value FROM kv_store WHERE key = ?")
                .await
        })?;

        let mut rows = block_on(async {
            stmt.query([key.to_string()]).await
        })?;

        if let Some(row) = block_on(async { rows.next().await })? {
            let value: String = block_on(async { row.get(0) })?;
            let deserialized: V = serde_json::from_str(&value)?;
            Ok(Some(deserialized))
        } else {
            Ok(None)
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()> {
        let serialized = serde_json::to_string(&value)?;

        block_on(async {
            self.conn
                .execute(
                    "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000",
                    [key.to_string(), serialized.clone(), serialized],
                )
                .await
        })?;

        Ok(())
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        block_on(async {
            self.conn
                .execute("DELETE FROM kv_store WHERE key = ?", [key.to_string()])
                .await
        })?;

        Ok(())
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        let mut stmt = block_on(async {
            self.conn
                .prepare("SELECT 1 FROM kv_store WHERE key = ? LIMIT 1")
                .await
        })?;

        let mut rows = block_on(async {
            stmt.query([key.to_string()]).await
        })?;

        Ok(block_on(async { rows.next().await })?.is_some())
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let (sql, param): (&str, Option<String>) = match prefix {
            Some(p) => (
                "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key",
                Some(format!("{p}%")),
            ),
            None => ("SELECT key FROM kv_store ORDER BY key", None),
        };

        let mut stmt = block_on(async { self.conn.prepare(sql).await })?;

        let rows = if let Some(p) = param {
            block_on(async { stmt.query([p]).await })?
        } else {
            block_on(async { stmt.query([turso::Value::Null; 0]).await })?
        };

        // Wrap rows in lazy iterator
        Ok(Box::new(TursoKeyIter::new(rows)))
    }
}

impl QueryStore for TursoStorage {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let turso_params = Self::to_turso_params(params);
        let mut stmt = block_on(async { self.conn.prepare(sql).await })?;
        let rows = block_on(async { stmt.query(turso_params).await })?;

        // Wrap rows in lazy iterator
        Ok(Box::new(TursoRowIter::new(rows)))
    }

    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let turso_params = Self::to_turso_params(params);
        let rows = block_on(async {
            self.conn
                .execute(sql, turso_params)
                .await
        })?;
        Ok(rows as u64)
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        block_on(async { self.conn.execute_batch(sql).await })?;
        Ok(())
    }
}

/// Lazy iterator for streaming keys from Turso query results.
struct TursoKeyIter {
    rows: turso::Rows,
}

impl TursoKeyIter {
    fn new(rows: turso::Rows) -> Self {
        Self { rows }
    }
}

impl Iterator for TursoKeyIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match block_on(async { self.rows.next().await }).ok()??.get(0).ok()? {
            Some(s) => Some(s),
            None => None,
        }
    }
}

/// Lazy iterator for streaming rows from Turso query results.
struct TursoRowIter {
    rows: turso::Rows,
}

impl TursoRowIter {
    fn new(rows: turso::Rows) -> Self {
        Self { rows }
    }
}

impl Iterator for TursoRowIter {
    type Item = SqlRow;

    fn next(&mut self) -> Option<Self::Item> {
        let row = block_on(async { self.rows.next().await }).ok()??;
        TursoStorage::turso_row_to_sql_row(&row).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_turso_storage_basic() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = TursoStorage::new(url).unwrap();
        storage.init_schema().unwrap();

        // Test set and get
        storage.set("test_key", "test_value").unwrap();

        let value: String = storage.get("test_key").unwrap().unwrap();
        assert_eq!(value, "test_value");

        // Test exists
        assert!(storage.exists("test_key").unwrap());
        assert!(!storage.exists("nonexistent").unwrap());

        // Test delete
        storage.delete("test_key").unwrap();
        assert!(!storage.exists("test_key").unwrap());
    }

    #[test]
    fn test_turso_storage_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = TursoStorage::new(url).unwrap();
        storage.init_schema().unwrap();

        storage.set("prefix:key1", "value1").unwrap();
        storage.set("prefix:key2", "value2").unwrap();
        storage.set("other:key3", "value3").unwrap();

        // List all keys
        let keys: Vec<String> = storage.list_keys(None).unwrap().collect();
        assert_eq!(keys.len(), 3);

        // List keys with prefix
        let keys: Vec<String> = storage.list_keys(Some("prefix:")).unwrap().collect();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_turso_storage_migrations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = db_path.to_str().unwrap();

        let storage = TursoStorage::new(url).unwrap();

        let migrations = &[
            ("001_create_users", "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL)"),
            ("002_create_sessions", "CREATE TABLE sessions (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)"),
        ];

        storage.migrate(migrations).unwrap();

        // Verify tables exist by querying sqlite_master
        let users_exist = storage
            .query("SELECT 1 FROM sqlite_master WHERE type='table' AND name='users'", &[])
            .unwrap()
            .next()
            .is_some();

        let sessions_exist = storage
            .query("SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions'", &[])
            .unwrap()
            .next()
            .is_some();

        let migrations_exist = storage
            .query("SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations'", &[])
            .unwrap()
            .next()
            .is_some();

        assert!(users_exist, "users table should be accessible");
        assert!(sessions_exist, "sessions table should be accessible");
        assert!(migrations_exist, "_migrations table should be accessible");
    }
}
