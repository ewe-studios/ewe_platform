//! Plan rules: group names, gaps, repeats and names.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::borrow::Cow;

use foundation_core::valtron::valtron_test;
use foundation_db::{
    Migration, MigrationError, MigrationProblem, MigrationRunner, MigrationSet, MigrationSource,
};
use foundation_vfs::EmptyFs;

use crate::common::{SHARED, TENANT_A};

static GAP: MigrationSet = MigrationSet {
    group: Cow::Borrowed("gap"),
    source: MigrationSource::Static(&[
        Migration {
            id: "001_first",
            name: "first",
            sql: "SELECT 1",
        },
        Migration {
            id: "002_second",
            name: "second",
            sql: "SELECT 2",
        },
        Migration {
            id: "004_fourth",
            name: "fourth",
            sql: "SELECT 4",
        },
    ]),
};

static DUPLICATE_NUMBER: MigrationSet = MigrationSet {
    group: Cow::Borrowed("dup_number"),
    source: MigrationSource::Static(&[
        Migration {
            id: "001_first",
            name: "first",
            sql: "SELECT 1",
        },
        Migration {
            id: "001_other",
            name: "other",
            sql: "SELECT 2",
        },
    ]),
};

static BAD_GROUP: MigrationSet = MigrationSet {
    group: Cow::Borrowed("Tenant-France"),
    source: MigrationSource::Static(&[Migration {
        id: "001_first",
        name: "first",
        sql: "SELECT 1",
    }]),
};

static BAD_ID: MigrationSet = MigrationSet {
    group: Cow::Borrowed("bad_id"),
    source: MigrationSource::Static(&[Migration {
        id: "01-bad",
        name: "bad",
        sql: "SELECT 1",
    }]),
};

fn find(error: &MigrationError, needle: &MigrationProblem) -> bool {
    error
        .problems
        .iter()
        .any(|problem| std::mem::discriminant(problem) == std::mem::discriminant(needle))
}

#[valtron_test]
async fn a_gap_is_reported_with_the_missing_number() {
    let error = MigrationRunner::from_static(&GAP)
        .plan(&EmptyFs::new())
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::Gap { missing: 3, .. })));
}

#[valtron_test]
async fn a_group_must_start_at_one() {
    static STARTS_AT_TWO: MigrationSet = MigrationSet {
        group: Cow::Borrowed("late"),
        source: MigrationSource::Static(&[Migration {
            id: "002_second",
            name: "second",
            sql: "SELECT 2",
        }]),
    };
    let error = MigrationRunner::from_static(&STARTS_AT_TWO)
        .plan(&EmptyFs::new())
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::Gap { missing: 1, .. })));
}

#[valtron_test]
async fn a_repeated_number_names_every_origin() {
    let error = MigrationRunner::from_static(&DUPLICATE_NUMBER)
        .plan(&EmptyFs::new())
        .await
        .unwrap_err();
    let duplicate = error
        .problems
        .iter()
        .find_map(|problem| match problem {
            MigrationProblem::DuplicateNumber { origins, .. } => Some(origins),
            _ => None,
        })
        .expect("a duplicate number problem");
    assert_eq!(duplicate.len(), 2);
    assert!(duplicate.iter().any(|origin| origin.contains("001_first")));
    assert!(duplicate.iter().any(|origin| origin.contains("001_other")));
}

#[valtron_test]
async fn a_bad_group_name_is_reported() {
    let error = MigrationRunner::from_static(&BAD_GROUP)
        .plan(&EmptyFs::new())
        .await
        .unwrap_err();
    assert!(find(
        &error,
        &MigrationProblem::BadGroupName {
            group: String::new()
        }
    ));
}

#[valtron_test]
async fn a_bad_identifier_is_reported() {
    let error = MigrationRunner::from_static(&BAD_ID)
        .plan(&EmptyFs::new())
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::BadName { .. })));
}

#[valtron_test]
async fn one_group_appears_only_once() {
    let runner = MigrationRunner::from_static(&SHARED).add_from_static(&SHARED);
    let error = runner.plan(&EmptyFs::new()).await.unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::DuplicateGroup { .. })));
}

#[valtron_test]
async fn the_same_number_in_two_groups_is_correct() {
    let runner = MigrationRunner::from_static(&SHARED).add_from_static(&TENANT_A);
    let plan = runner.plan(&EmptyFs::new()).await.unwrap();
    assert_eq!(plan.len(), 4);
    let groups: Vec<&str> = plan
        .iter()
        .map(|migration| migration.group.as_str())
        .collect();
    assert_eq!(groups, vec!["shared", "shared", "tenant_a", "tenant_a"]);
    // Both groups use number 1, and that is correct.
    assert_eq!(plan[0].number, 1);
    assert_eq!(plan[2].number, 1);
}

#[valtron_test]
async fn every_problem_is_collected_in_one_error() {
    let runner = MigrationRunner::from_static(&GAP)
        .add_from_static(&BAD_GROUP)
        .add_from_static(&DUPLICATE_NUMBER);
    let error = runner.plan(&EmptyFs::new()).await.unwrap_err();
    assert!(error.problems.len() >= 3, "problems: {:?}", error.problems);
    assert!(find(
        &error,
        &MigrationProblem::BadGroupName {
            group: String::new()
        }
    ));
}
