use foundation_db::{Migration, MigrationRunner};
use foundation_db::traits::QueryStore;

use crate::store::VectorStoreError;

pub static MIGRATIONS: &[Migration] = &[
    Migration {
        id: "vectors_001_create_vectors",
        name: "Create vectors table for VectorStore",
        sql: include_str!("sql/001_create_vectors.sql"),
    },
];

pub fn run_migrations(store: &dyn QueryStore) -> Result<usize, VectorStoreError> {
    MigrationRunner::new(MIGRATIONS)
        .run(store)
        .map_err(|e| VectorStoreError::Backend(format!("migration failed: {e}")))
}
