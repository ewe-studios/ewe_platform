//! Record-table isolation between groups.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_core::valtron::valtron_test;
use foundation_db::{MigrationConfig, MigrationProblem, MigrationRunner};
use foundation_vfs::EmptyFs;

use crate::common::{insert_record, memory_store, rows, SHARED, TENANT_A};

#[valtron_test]
async fn two_groups_with_the_same_numbers_get_separate_rows() {
    let store = memory_store().await;
    let runner = MigrationRunner::from_static(&SHARED).add_from_static(&TENANT_A);
    runner.run_async(&EmptyFs::new(), &store).await.unwrap();

    let table = rows(
        &store,
        "SELECT group_name, number FROM _migrations ORDER BY group_name, number",
    )
    .await;
    assert_eq!(table.len(), 4);
    let group_names: Vec<String> = table
        .iter()
        .map(|row| row.get::<String>(0).unwrap())
        .collect();
    assert_eq!(
        group_names,
        vec!["shared", "shared", "tenant_a", "tenant_a"]
    );
    let numbers: Vec<i64> = table.iter().map(|row| row.get::<i64>(1).unwrap()).collect();
    assert_eq!(numbers, vec![1, 2, 1, 2]);
}

#[valtron_test]
async fn an_unknown_group_is_reported() {
    let store = memory_store().await;
    let runner = MigrationRunner::from_static(&SHARED);
    runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    insert_record(&store, "tenant_2", 1, "create_reports", "").await;

    let summary = runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    assert_eq!(summary.unknown_groups, vec!["tenant_2"]);
    assert!(summary.applied.is_empty());
}

#[valtron_test]
async fn a_strict_runner_rejects_an_unknown_group() {
    let store = memory_store().await;
    let runner = MigrationRunner::from_static(&SHARED);
    runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    insert_record(&store, "tenant_2", 1, "create_reports", "").await;

    let strict = MigrationRunner::from_static(&SHARED)
        .with_config(MigrationConfig::default().with_strict_groups(true));
    let error = strict.run_async(&EmptyFs::new(), &store).await.unwrap_err();
    assert!(error.problems.iter().any(|problem| matches!(
        problem,
        MigrationProblem::UnknownGroup { group } if group == "tenant_2"
    )));
}
