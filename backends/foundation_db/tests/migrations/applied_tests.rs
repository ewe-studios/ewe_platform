//! Rules against the record table: checksums, names and gaps.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::borrow::Cow;

use foundation_core::valtron::valtron_test;
use foundation_db::{
    Migration, MigrationConfig, MigrationProblem, MigrationRunner, MigrationSet, MigrationSource,
};
use foundation_vfs::EmptyFs;

use crate::common::{insert_record, memory_store};

static BASE: MigrationSet = MigrationSet {
    group: Cow::Borrowed("applied"),
    source: MigrationSource::Static(&[Migration {
        id: "001_add_email",
        name: "add email",
        sql: "CREATE TABLE IF NOT EXISTS applied_t (id INTEGER PRIMARY KEY)",
    }]),
};

static CHANGED_SQL: MigrationSet = MigrationSet {
    group: Cow::Borrowed("applied"),
    source: MigrationSource::Static(&[Migration {
        id: "001_add_email",
        name: "add email",
        sql: "CREATE TABLE IF NOT EXISTS applied_t (id INTEGER PRIMARY KEY, email TEXT)",
    }]),
};

static CHANGED_NAME: MigrationSet = MigrationSet {
    group: Cow::Borrowed("applied"),
    source: MigrationSource::Static(&[Migration {
        id: "001_add_phone",
        name: "add phone",
        sql: "CREATE TABLE IF NOT EXISTS applied_t (id INTEGER PRIMARY KEY)",
    }]),
};

static TWO: MigrationSet = MigrationSet {
    group: Cow::Borrowed("applied"),
    source: MigrationSource::Static(&[
        Migration {
            id: "001_first",
            name: "first",
            sql: "CREATE TABLE IF NOT EXISTS first_t (id INTEGER PRIMARY KEY)",
        },
        Migration {
            id: "002_second",
            name: "second",
            sql: "CREATE TABLE IF NOT EXISTS second_t (id INTEGER PRIMARY KEY)",
        },
    ]),
};

#[valtron_test]
async fn a_changed_checksum_is_reported() {
    let store = memory_store().await;
    MigrationRunner::from_static(&BASE)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap();
    let error = MigrationRunner::from_static(&CHANGED_SQL)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::ChecksumChanged { number: 1, .. })));
}

#[valtron_test]
async fn checksum_verification_can_be_off() {
    let store = memory_store().await;
    MigrationRunner::from_static(&BASE)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap();
    let runner = MigrationRunner::from_static(&CHANGED_SQL)
        .with_config(MigrationConfig::default().with_verify_checksums(false));
    let summary = runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    assert!(summary.applied.is_empty());
}

#[valtron_test]
async fn a_changed_name_is_reported() {
    let store = memory_store().await;
    MigrationRunner::from_static(&BASE)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap();
    let error = MigrationRunner::from_static(&CHANGED_NAME)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap_err();
    assert!(error.problems.iter().any(|problem| matches!(
        problem,
        MigrationProblem::NameChanged {
            number: 1,
            ref applied,
            ref planned,
            ..
        } if applied == "add email" && planned == "add phone"
    )));
}

#[valtron_test]
async fn applied_numbers_must_form_a_prefix() {
    let store = memory_store().await;
    let runner = MigrationRunner::from_static(&TWO);
    runner.run_async(&EmptyFs::new(), &store).await.unwrap();
    insert_record(&store, "applied", 4, "fourth", "").await;

    let error = runner.run_async(&EmptyFs::new(), &store).await.unwrap_err();
    assert!(error.problems.iter().any(|problem| matches!(
        problem,
        MigrationProblem::AppliedNotPrefix { ref missing, .. } if missing == &vec![3]
    )));
}

#[valtron_test]
async fn an_applied_number_must_stay_in_the_plan() {
    let store = memory_store().await;
    MigrationRunner::from_static(&TWO)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap();
    let error = MigrationRunner::from_static(&BASE)
        .run_async(&EmptyFs::new(), &store)
        .await
        .unwrap_err();
    assert!(error.problems.iter().any(|problem| matches!(
        problem,
        MigrationProblem::AppliedButMissing { number: 2, .. }
    )));
}
