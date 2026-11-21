//! Upgrade of the old `_migrations` table, with the 023 repair.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_core::valtron::valtron_test;
use foundation_db::{DataValue, MigrationRunner, QueryStore, DB_CORE, DB_CORE_GROUP, MIGRATIONS};
use foundation_vfs::EmptyFs;

use crate::common::{count, has_column, memory_store, rows, table_exists};

fn number(id: &str) -> u64 {
    id.split('_').next().unwrap().parse().unwrap()
}

/// Build the table shape and the record rows of a database from today's code:
/// migrations 1 to 22, then 24 and 25, with no 023.
async fn old_database(store: &dyn QueryStore) {
    store
        .execute_batch(
            "CREATE TABLE _migrations (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 applied_at INTEGER DEFAULT 0
             )",
        )
        .await
        .unwrap();
    for migration in MIGRATIONS {
        let number = number(migration.id);
        if number > 25 || number == 23 {
            continue;
        }
        store.execute_batch(migration.sql).await.unwrap();
        store
            .execute(
                "INSERT INTO _migrations (id, name, applied_at) VALUES (?, ?, 0)",
                &[
                    DataValue::Text(migration.id.to_string()),
                    DataValue::Text(migration.name.to_string()),
                ],
            )
            .await
            .unwrap();
    }
}

#[valtron_test]
async fn the_old_table_becomes_db_core_and_023_is_repaired() {
    let store = memory_store().await;
    old_database(&store).await;
    assert!(
        !table_exists(&store, "vectors").await,
        "the old database has no vectors table"
    );

    let summary = MigrationRunner::from_static(&DB_CORE)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap();

    assert!(
        has_column(&store, "_migrations", "group_name").await,
        "the record table must take the new shape"
    );
    assert!(table_exists(&store, "vectors").await);
    assert_eq!(
        summary.applied,
        vec![
            (DB_CORE_GROUP.to_string(), 23),
            (DB_CORE_GROUP.to_string(), 26),
            (DB_CORE_GROUP.to_string(), 27),
            (DB_CORE_GROUP.to_string(), 28),
        ]
    );
    assert_eq!(count(&store, "SELECT COUNT(*) FROM _migrations").await, 28);
    let groups: Vec<String> = rows(&store, "SELECT DISTINCT group_name FROM _migrations")
        .await
        .iter()
        .map(|row| row.get::<String>(0).unwrap())
        .collect();
    assert_eq!(groups, vec![DB_CORE_GROUP.to_string()]);
    let numbers: Vec<i64> = rows(&store, "SELECT number FROM _migrations ORDER BY number")
        .await
        .iter()
        .map(|row| row.get::<i64>(0).unwrap())
        .collect();
    assert_eq!(numbers, (1..=28).collect::<Vec<i64>>());
}

#[valtron_test]
async fn the_second_run_applies_nothing_after_the_upgrade() {
    let store = memory_store().await;
    old_database(&store).await;
    let runner = MigrationRunner::from_static(&DB_CORE);
    runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    let summary = runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    assert!(summary.applied.is_empty());
}
