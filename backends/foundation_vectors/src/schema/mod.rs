use foundation_db::{AsyncQueryStore, Migration, MigrationRunner};

use crate::store::VectorStoreError;

pub static MIGRATIONS: &[Migration] = &[
    Migration {
        id: "vectors_001_create_vectors",
        name: "Create vectors table for VectorStore",
        sql: include_str!("sql/001_create_vectors.sql"),
    },
];

/// Apply the vector-store migrations that haven't run yet.
///
/// # Errors
///
/// Returns [`VectorStoreError::Backend`] if a migration fails.
pub async fn run_migrations(store: &dyn AsyncQueryStore) -> Result<usize, VectorStoreError> {
    MigrationRunner::new(MIGRATIONS)
        .run_async(store)
        .await
        .map_err(|e| VectorStoreError::Backend(format!("migration failed: {e}")))
}
