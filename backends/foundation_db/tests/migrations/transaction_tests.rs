//! One transaction for a migration and its record row.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::borrow::Cow;

use foundation_core::valtron::valtron_test;
use foundation_db::{Migration, MigrationProblem, MigrationRunner, MigrationSet, MigrationSource};
use foundation_vfs::EmptyFs;

use crate::common::{count, memory_store, table_exists};

static FAILING: MigrationSet = MigrationSet {
    group: Cow::Borrowed("transaction"),
    source: MigrationSource::Static(&[Migration {
        id: "001_create_then_fail",
        name: "create then fail",
        sql: "CREATE TABLE IF NOT EXISTS tx_created (id INTEGER PRIMARY KEY); \
              INSERT INTO no_such_table (id) VALUES (1);",
    }]),
};

#[valtron_test]
async fn a_failed_migration_leaves_no_record_and_no_table() {
    let store = memory_store().await;
    let error = MigrationRunner::from_static(&FAILING)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::Apply { number: 1, .. })));
    assert!(
        !table_exists(&store, "tx_created").await,
        "the first statement of the failed migration must roll back"
    );
    assert_eq!(
        count(&store, "SELECT COUNT(*) FROM _migrations").await,
        0,
        "a failed migration must not be recorded"
    );
}

#[cfg(feature = "libsql-local")]
#[valtron_test]
async fn a_failed_migration_rolls_back_on_libsql() {
    let store = foundation_db::LibsqlStore::new_kv(":memory:", None, crate::common::ctx())
        .await
        .unwrap();
    let error = MigrationRunner::from_static(&FAILING)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::Apply { number: 1, .. })));
    assert!(!table_exists(&store, "tx_created").await);
    assert_eq!(count(&store, "SELECT COUNT(*) FROM _migrations").await, 0);
}
