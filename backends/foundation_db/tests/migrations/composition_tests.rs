//! Runner composition: one base, many databases.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_core::valtron::valtron_test;
use foundation_db::MigrationRunner;
use foundation_vfs::EmptyFs;

use crate::common::{memory_store, SHARED, TENANT_A, TENANT_B};

#[valtron_test]
async fn one_base_builds_two_runners_and_stays_unchanged() {
    let shared = MigrationRunner::from_static(&SHARED);
    let france = shared.add_from_static(&TENANT_A);
    let tenant_2 = shared.add_from_static(&TENANT_B);

    assert_eq!(shared.groups(), vec!["shared"]);
    assert_eq!(france.groups(), vec!["shared", "tenant_a"]);
    assert_eq!(tenant_2.groups(), vec!["shared", "tenant_b"]);
}

#[valtron_test]
async fn groups_apply_in_runner_order() {
    let store = memory_store().await;
    let runner = MigrationRunner::from_static(&SHARED).add_from_static(&TENANT_A);
    let summary = runner.run_async(&EmptyFs::new(), &store).await.unwrap();

    assert_eq!(
        summary.applied,
        vec![
            ("shared".to_string(), 1),
            ("shared".to_string(), 2),
            ("tenant_a".to_string(), 1),
            ("tenant_a".to_string(), 2),
        ]
    );
}

#[valtron_test]
async fn the_second_run_applies_nothing() {
    let store = memory_store().await;
    let runner = MigrationRunner::from_static(&SHARED).add_from_static(&TENANT_A);
    runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    let summary = runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    assert!(summary.applied.is_empty());
}
