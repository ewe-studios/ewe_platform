//! Turso/libsql storage backend implementation.

use async_trait::async_trait;
use libsql::{Database, params};
use serde::{de::DeserializeOwned, Serialize};

use crate::errors::StorageResult;
use crate::storage_provider::{KeyValueStore, QueryStore};

/// Turso/libsql storage backend.
pub struct TursoStorage {
    db: Database,
    conn: libsql::Connection,
}

impl TursoStorage {
    /// Create a new Turso storage connection.
    #[allow(clippy::unused_async)]
    pub async fn new(url: &str) -> StorageResult<Self> {
        let db = Database::open(url)?;
        let conn = db.connect()?;

        Ok(Self { db, conn })
    }

    /// Initialize the database schema.
    pub async fn init_schema(&self) -> StorageResult<()> {
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

        self.conn
            .execute_batch(schema_sql)
            .await?;

        Ok(())
    }

    /// Run database migrations.
    pub async fn migrate(&self, migrations: &[(&str, &str)]) -> StorageResult<()> {
        // Create migrations table if it doesn't exist
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at INTEGER DEFAULT (strftime('%s', 'now') * 1000)
            )"
        ).await?;

        for (id, sql) in migrations {
            // Check if migration already applied
            let mut stmt = self
                .conn
                .prepare("SELECT 1 FROM _migrations WHERE id = ?")
                .await?;

            let mut rows = stmt.query([id.to_string()]).await?;
            let exists = rows.next().await?.is_some();

            if !exists {
                // Apply migration
                self.conn.execute_batch(sql).await?;

                // Record migration
                self.conn
                    .execute(
                        "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                        [id.to_string(), id.to_string()],
                    )
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl KeyValueStore for TursoStorage {
    async fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM kv_store WHERE key = ?")
            .await?;

        let mut rows = stmt.query([key.to_string()]).await?;

        if let Some(row) = rows.next().await? {
            let value: String = row.get(0)?;
            let deserialized: V = serde_json::from_str(&value)?;
            Ok(Some(deserialized))
        } else {
            Ok(None)
        }
    }

    async fn set<V: Serialize + Send>(&self, key: &str, value: V) -> StorageResult<()> {
        let serialized = serde_json::to_string(&value)?;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000)",
                params![key, serialized],
            )
            .await?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.conn
            .execute("DELETE FROM kv_store WHERE key = ?", [key.to_string()])
            .await?;

        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT 1 FROM kv_store WHERE key = ? LIMIT 1")
            .await?;

        let mut rows = stmt.query([key.to_string()]).await?;
        Ok(rows.next().await?.is_some())
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        let (sql, param): (&str, Option<String>) = match prefix {
            Some(p) => (
                "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key",
                Some(format!("{p}%")),
            ),
            None => ("SELECT key FROM kv_store ORDER BY key", None),
        };

        let mut stmt = self.conn.prepare(sql).await?;

        let mut rows = if let Some(p) = param {
            stmt.query([p]).await?
        } else {
            stmt.query(params![]).await?
        };

        let mut keys = Vec::new();
        while let Some(row) = rows.next().await? {
            let key: String = row.get(0)?;
            keys.push(key);
        }

        Ok(keys)
    }
}

#[async_trait]
impl QueryStore for TursoStorage {
    async fn query(
        &self,
        sql: &str,
        params_vec: Vec<libsql::Value>,
    ) -> StorageResult<Vec<libsql::Row>> {
        let mut stmt = self.conn.prepare(sql).await?;
        let mut rows = stmt.query(params_vec).await?;

        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            results.push(row);
        }

        Ok(results)
    }

    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> StorageResult<u64> {
        let rows = self.conn.execute(sql, params).await?;
        Ok(rows as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_turso_storage_basic() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = format!("file:{}", db_path.display());

        let storage = TursoStorage::new(&url).await.unwrap();
        storage.init_schema().await.unwrap();

        // Test set and get
        storage
            .set("test_key", "test_value")
            .await
            .unwrap();

        let value: String = storage.get("test_key").await.unwrap().unwrap();
        assert_eq!(value, "test_value");

        // Test exists
        assert!(storage.exists("test_key").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Test delete
        storage.delete("test_key").await.unwrap();
        assert!(!storage.exists("test_key").await.unwrap());
    }

    #[tokio::test]
    async fn test_turso_storage_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = format!("file:{}", db_path.display());

        let storage = TursoStorage::new(&url).await.unwrap();
        storage.init_schema().await.unwrap();

        storage.set("prefix:key1", "value1").await.unwrap();
        storage.set("prefix:key2", "value2").await.unwrap();
        storage.set("other:key3", "value3").await.unwrap();

        // List all keys
        let keys = storage.list_keys(None).await.unwrap();
        assert_eq!(keys.len(), 3);

        // List keys with prefix
        let keys = storage.list_keys(Some("prefix:")).await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_turso_storage_migrations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let url = format!("file:{}", db_path.display());

        let storage = TursoStorage::new(&url).await.unwrap();

        let migrations = &[
            ("001_create_users", "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL)"),
            ("002_create_sessions", "CREATE TABLE sessions (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)"),
        ];

        storage.migrate(migrations).await.unwrap();

        // Verify tables were created by checking they're queryable
        let count_result = storage
            .query("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('users', 'sessions', '_migrations')", vec![])
            .await
            .unwrap();

        // Also verify individual tables exist by querying them
        let users_exist = storage
            .query("SELECT 1 FROM users LIMIT 1", vec![])
            .await
            .is_ok();

        let sessions_exist = storage
            .query("SELECT 1 FROM sessions LIMIT 1", vec![])
            .await
            .is_ok();

        let migrations_exist = storage
            .query("SELECT 1 FROM _migrations LIMIT 1", vec![])
            .await
            .is_ok();

        // Check that expected tables exist by verifying they're queryable
        assert!(users_exist, "users table should be accessible");
        assert!(sessions_exist, "sessions table should be accessible");
        assert!(migrations_exist, "_migrations table should be accessible");
    }
}
