//! Database migrations for `foundation_db`.
//!
//! Migration runner uses synchronous [`QueryStore`] trait with [`DataValue`] params.
//! For async backends (D1/wasm), use [`MigrationRunner::run_async`] with [`AsyncQueryStore`].

use crate::core::errors::StorageResult;
use crate::core::storage_provider::{AsyncQueryStore, DataValue, QueryStore};

/// A single database migration.
pub struct Migration {
    pub id: &'static str,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations in order.
pub static MIGRATIONS: &[Migration] = &[
    Migration {
        id: "001_create_kv_store",
        name: "Create key-value store table",
        sql: include_str!("sql/001_create_kv_store.sql"),
    },
    Migration {
        id: "002_create_users",
        name: "Create users table",
        sql: include_str!("sql/002_create_users.sql"),
    },
    Migration {
        id: "003_create_sessions",
        name: "Create sessions table",
        sql: include_str!("sql/003_create_sessions.sql"),
    },
    Migration {
        id: "004_create_accounts",
        name: "Create accounts table for OAuth",
        sql: include_str!("sql/004_create_accounts.sql"),
    },
    Migration {
        id: "005_create_verification_tokens",
        name: "Create verification tokens table",
        sql: include_str!("sql/005_create_verification_tokens.sql"),
    },
    Migration {
        id: "006_create_oauth_credentials",
        name: "Create OAuth credentials table",
        sql: include_str!("sql/006_create_oauth_credentials.sql"),
    },
    Migration {
        id: "007_create_oauth_states",
        name: "Create OAuth states table for PKCE",
        sql: include_str!("sql/007_create_oauth_states.sql"),
    },
    Migration {
        id: "008_create_jwt_tokens",
        name: "Create JWT tokens table",
        sql: include_str!("sql/008_create_jwt_tokens.sql"),
    },
    Migration {
        id: "009_create_auth_states",
        name: "Create auth states table",
        sql: include_str!("sql/009_create_auth_states.sql"),
    },
    Migration {
        id: "010_create_api_keys",
        name: "Create API keys table",
        sql: include_str!("sql/010_create_api_keys.sql"),
    },
    Migration {
        id: "011_create_two_factor",
        name: "Create two-factor authentication tables",
        sql: include_str!("sql/011_create_two_factor.sql"),
    },
    Migration {
        id: "012_create_email_otps",
        name: "Create email OTPs table",
        sql: include_str!("sql/012_create_email_otps.sql"),
    },
    Migration {
        id: "013_create_magic_links",
        name: "Create magic links table",
        sql: include_str!("sql/013_create_magic_links.sql"),
    },
    Migration {
        id: "014_create_rate_limits",
        name: "Create rate limits table",
        sql: include_str!("sql/014_create_rate_limits.sql"),
    },
    Migration {
        id: "015_create_audit_logs",
        name: "Create audit logs table",
        sql: include_str!("sql/015_create_audit_logs.sql"),
    },
    Migration {
        id: "016_create_oauth_clients",
        name: "Create OAuth clients table",
        sql: include_str!("sql/016_create_oauth_clients.sql"),
    },
    Migration {
        id: "017_create_authorization_codes",
        name: "Create authorization codes table",
        sql: include_str!("sql/017_create_authorization_codes.sql"),
    },
    Migration {
        id: "018_create_refresh_tokens",
        name: "Create refresh tokens table",
        sql: include_str!("sql/018_create_refresh_tokens.sql"),
    },
    Migration {
        id: "019_create_device_codes",
        name: "Create device codes table",
        sql: include_str!("sql/019_create_device_codes.sql"),
    },
    Migration {
        id: "020_create_documents",
        name: "Create documents table for DocumentStore",
        sql: include_str!("sql/020_create_documents.sql"),
    },
    Migration {
        id: "021_promote_document_columns",
        name: "Add promoted searchable columns to documents",
        sql: include_str!("sql/021_promote_document_columns.sql"),
    },
];

/// Migration runner that applies pending migrations.
pub struct MigrationRunner<'a> {
    migrations: &'a [Migration],
}

impl<'a> MigrationRunner<'a> {
    /// Create a new migration runner.
    #[must_use]
    pub fn new(migrations: &'a [Migration]) -> Self {
        Self { migrations }
    }

    /// Run all pending migrations asynchronously.
    ///
    /// This is the preferred method for wasm backends (D1) where the underlying
    /// JS APIs are Promise-based.
    ///
    /// # Errors
    ///
    /// Returns an error if any migration SQL statement fails.
    pub async fn run_async(&self, store: &dyn AsyncQueryStore) -> StorageResult<usize> {
        // Ensure the migrations tracking table exists
        store.execute_batch_async(
            "CREATE TABLE IF NOT EXISTS _migrations (id TEXT PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER DEFAULT (strftime('%s', 'now') * 1000))"
        ).await?;

        let mut count = 0;

        for migration in self.migrations {
            // Check if migration already applied
            let rows = store.query_async(
                "SELECT 1 FROM _migrations WHERE id = ?",
                &[DataValue::Text(migration.id.to_string())],
            ).await?;

            let exists = !rows.collect_all().await?.is_empty();

            if !exists {
                // Apply migration
                store.execute_batch_async(migration.sql).await?;

                // Record migration
                store.execute_async(
                    "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                    &[
                        DataValue::Text(migration.id.to_string()),
                        DataValue::Text(migration.name.to_string()),
                    ],
                ).await?;

                count += 1;
            }
        }

        Ok(count)
    }

    /// Run all pending migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if any migration SQL statement fails.
    pub fn run(&self, store: &dyn QueryStore) -> StorageResult<usize> {
        let mut count = 0;

        for migration in self.migrations {
            // Check if migration already applied
            let mut rows = store.query(
                "SELECT 1 FROM _migrations WHERE id = ?",
                &[DataValue::Text(migration.id.to_string())],
            )?;

            // Check if any rows returned (migration exists)
            let exists = rows.next().is_some();

            if !exists {
                // Apply migration - consume the iterator
                store.execute_batch(migration.sql)?;

                // Record migration
                store.execute(
                    "INSERT OR IGNORE INTO _migrations (name) VALUES (?)",
                    &[DataValue::Text(migration.id.to_string())],
                )?;

                count += 1;
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_defined() {
        assert!(!MIGRATIONS.is_empty());
        assert_eq!(MIGRATIONS.len(), 21);
    }

    #[test]
    fn test_migration_ids_unique() {
        let ids: Vec<&str> = MIGRATIONS.iter().map(|m| m.id).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique_ids.len(), "Migration IDs must be unique");
    }
}
