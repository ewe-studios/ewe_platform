use std::borrow::Cow;

use foundation_db::{
    EmptyFs, Migration, MigrationRunner, MigrationSet, MigrationSource, QueryStore,
};

use crate::store::VectorStoreError;

pub static MIGRATIONS: &[Migration] = &[Migration {
    id: "001_create_vectors",
    name: "Create vectors table for VectorStore",
    sql: include_str!("sql/001_create_vectors.sql"),
}];

/// The `vectors` migration group of this crate.
pub static VECTORS: MigrationSet = MigrationSet {
    group: Cow::Borrowed("vectors"),
    source: MigrationSource::Static(MIGRATIONS),
};

/// Apply the vector-store migrations that haven't run yet.
///
/// # Errors
///
/// Returns [`VectorStoreError::Backend`] if a migration fails.
pub async fn run_migrations(store: &dyn QueryStore) -> Result<usize, VectorStoreError> {
    let summary = MigrationRunner::from_static(&VECTORS)
        .run_async(&EmptyFs::new(), store)
        .await
        .map_err(|e| VectorStoreError::Backend(format!("migration failed: {e}")))?;
    Ok(summary.applied.len())
}
