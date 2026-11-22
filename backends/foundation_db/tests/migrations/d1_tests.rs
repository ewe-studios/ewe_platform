//! Migration groups against Cloudflare D1 through the local wrangler API.
//!
//! The tests run only when `CF_INTEGRATION_TEST=1` and `wrangler dev --local`
//! answers on the local explorer API. See `tests/common/mod.rs`.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

#[path = "../common/mod.rs"]
mod cf_common;

use std::borrow::Cow;

use foundation_core::valtron::valtron_test;
use foundation_db::{Migration, MigrationConfig, MigrationRunner, MigrationSet, MigrationSource};
use foundation_vfs::EmptyFs;

use crate::common::{count, table_exists, TENANT_B};

static FAILING: MigrationSet = MigrationSet {
    group: Cow::Borrowed("d1_transaction"),
    source: MigrationSource::Static(&[Migration {
        id: "001_create_then_fail",
        name: "create then fail",
        sql: "CREATE TABLE IF NOT EXISTS tx_created (id INTEGER PRIMARY KEY); \
              INSERT INTO no_such_table (id) VALUES (1);",
    }]),
};

fn config() -> MigrationConfig {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    MigrationConfig::default().with_table(format!("_migrations_{}_{stamp}", std::process::id()))
}

#[valtron_test]
async fn d1_applies_each_migration_once() {
    let Some(store) = cf_common::make_d1_store() else {
        println!("Skipping D1 migration test - miniflare not available");
        return;
    };
    let config = config();
    let table = config.table.clone();
    let runner = MigrationRunner::from_static(&crate::common::SHARED)
        .add_from_static(&TENANT_B)
        .with_config(config);

    let summary = runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    assert_eq!(summary.applied.len(), 3);
    assert_eq!(
        count(&store, &format!("SELECT COUNT(*) FROM {table}")).await,
        3
    );

    let second = runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    assert_eq!(second.applied, Vec::new());
}

#[valtron_test]
async fn d1_rolls_back_a_failed_migration() {
    let Some(store) = cf_common::make_d1_store() else {
        println!("Skipping D1 migration test - miniflare not available");
        return;
    };
    let config = config();
    let table = config.table.clone();
    let runner = MigrationRunner::from_static(&FAILING).with_config(config);
    assert!(runner.run_async(&EmptyFs::new(), &store).await.is_err());
    assert!(!table_exists(&store, "tx_created").await);
    assert_eq!(
        count(&store, &format!("SELECT COUNT(*) FROM {table}")).await,
        0
    );
}
