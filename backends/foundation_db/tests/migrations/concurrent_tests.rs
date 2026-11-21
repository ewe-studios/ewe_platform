//! Two runners on one database apply each migration once.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::borrow::Cow;
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_db::{Migration, MigrationRunner, MigrationSet, MigrationSource};
use foundation_vfs::EmptyFs;

use crate::common::{count, memory_store};

static CONCURRENT: MigrationSet = MigrationSet {
    group: Cow::Borrowed("concurrent"),
    source: MigrationSource::Static(&[
        Migration {
            id: "001_create_counter",
            name: "create counter",
            sql: "CREATE TABLE IF NOT EXISTS counter (id INTEGER PRIMARY KEY); \
                  INSERT INTO counter (id) VALUES (1);",
        },
        Migration {
            id: "002_create_notes",
            name: "create notes",
            sql: "CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY);",
        },
    ]),
};

#[valtron_test]
async fn two_runners_apply_each_migration_once() {
    let store = Arc::new(memory_store().await);
    let runner = MigrationRunner::from_static(&CONCURRENT);
    let files = EmptyFs::new();

    let first = runner.run_async(&files, &*store);
    let second = runner.run_async(&files, &*store);
    let (first, second) = futures_lite::future::zip(first, second).await;
    first.unwrap();
    second.unwrap();

    assert_eq!(
        count(&*store, "SELECT COUNT(*) FROM _migrations").await,
        2,
        "each migration must have one record row"
    );
    assert_eq!(
        count(&*store, "SELECT COUNT(*) FROM counter").await,
        1,
        "the migration SQL must run only once"
    );
}
