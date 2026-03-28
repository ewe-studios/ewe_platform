//! Database migrations for `foundation_db`.
//!
//! Migration runner uses synchronous [`QueryStore`] trait with [`DataValue`] params.

use crate::storage_provider::{DataValue, QueryStore};
use crate::errors::StorageResult;

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
                for _result in store.execute_batch(migration.sql)? {}

                // Record migration - consume the iterator
                for _result in store.execute(
                    "INSERT INTO _migrations (id, name) VALUES (?, ?)",
                    &[
                        DataValue::Text(migration.id.to_string()),
                        DataValue::Text(migration.name.to_string()),
                    ],
                )? {}

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
        assert_eq!(MIGRATIONS.len(), 15);
    }

    #[test]
    fn test_migration_ids_unique() {
        let ids: Vec<&str> = MIGRATIONS.iter().map(|m| m.id).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique_ids.len(), "Migration IDs must be unique");
    }
}
