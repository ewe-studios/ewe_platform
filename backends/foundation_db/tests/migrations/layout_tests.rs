//! Directory layouts: one folder is one group.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_core::valtron::valtron_test;
use foundation_db::{
    embed_migration_layout, EmbeddedLayout, MigrationConfig, MigrationLayout,
    MigrationLayoutConfig, MigrationProblem, PlannedMigration,
};
use foundation_nativeapis::MemoryFs;

use crate::common::{memory_fs, mkdir, write};

fn config() -> MigrationConfig {
    MigrationConfig::default()
}

async fn database_fs() -> MemoryFs {
    let fs = memory_fs();
    mkdir(&fs, "/db").await;
    for group in ["shared", "france", "tenant_2", "auth"] {
        mkdir(&fs, &format!("/db/{group}")).await;
    }
    write(
        &fs,
        "/db/shared/0001_create_users.sql",
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY)",
    )
    .await;
    write(
        &fs,
        "/db/shared/0002_create_sessions.sql",
        "CREATE TABLE IF NOT EXISTS sessions (id INTEGER PRIMARY KEY)",
    )
    .await;
    write(
        &fs,
        "/db/france/0001_create_invoices.sql",
        "CREATE TABLE IF NOT EXISTS invoices (id INTEGER PRIMARY KEY)",
    )
    .await;
    write(
        &fs,
        "/db/tenant_2/0001_create_reports.sql",
        "CREATE TABLE IF NOT EXISTS reports (id INTEGER PRIMARY KEY)",
    )
    .await;
    write(
        &fs,
        "/db/auth/0001_create_keys.sql",
        "CREATE TABLE IF NOT EXISTS keys (id INTEGER PRIMARY KEY)",
    )
    .await;
    fs
}

#[valtron_test]
async fn folder_names_become_groups() {
    let fs = database_fs().await;
    let layout = MigrationLayout::scan(&fs, "/db", MigrationLayoutConfig::default(), config())
        .await
        .unwrap();

    assert_eq!(
        layout.runner_for("france").groups(),
        vec!["shared", "france"]
    );
    assert_eq!(
        layout.runner_for("tenant_2").groups(),
        vec!["shared", "tenant_2"]
    );
    assert_eq!(layout.runner_for("nobody").groups(), vec!["shared"]);
    let databases: Vec<&str> = layout.databases().collect();
    assert_eq!(databases, vec!["auth", "france", "tenant_2"]);
}

#[valtron_test]
async fn templates_apply_first_in_config_order() {
    let fs = database_fs().await;
    let layout = MigrationLayout::scan(
        &fs,
        "/db",
        MigrationLayoutConfig::default()
            .with_template_groups(vec!["auth".to_string(), "shared".to_string()]),
        config(),
    )
    .await
    .unwrap();

    assert_eq!(
        layout.runner_for("france").groups(),
        vec!["auth", "shared", "france"]
    );
}

#[valtron_test]
async fn a_missing_template_folder_is_reported() {
    let fs = database_fs().await;
    let error = MigrationLayout::scan(
        &fs,
        "/db",
        MigrationLayoutConfig::default().with_template_groups(vec!["missing".to_string()]),
        config(),
    )
    .await
    .unwrap_err();
    assert!(error.problems.iter().any(|problem| matches!(
        problem,
        MigrationProblem::Io { ref origin, .. } if origin.ends_with("missing")
    )));
}

#[valtron_test]
async fn a_folder_inside_a_group_is_reported() {
    let fs = database_fs().await;
    mkdir(&fs, "/db/shared/nested").await;
    let error = MigrationLayout::scan(&fs, "/db", MigrationLayoutConfig::default(), config())
        .await
        .unwrap_err();
    assert!(error
        .problems
        .iter()
        .any(|problem| matches!(problem, MigrationProblem::NestedFolder { .. })));
}

static EMBEDDED_LAYOUT: EmbeddedLayout =
    embed_migration_layout!("tests/migrations/data/database", templates = ["shared"]);

fn fingerprint(plan: &[PlannedMigration]) -> Vec<(String, u64, String, String)> {
    plan.iter()
        .map(|migration| {
            (
                migration.group.clone(),
                migration.number,
                migration.name.clone(),
                migration.checksum.clone(),
            )
        })
        .collect()
}

#[valtron_test]
async fn an_embedded_layout_gives_the_same_runners_as_scan() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/migrations/data/database");
    let fs = memory_fs();
    mkdir(&fs, "database").await;
    for group in ["shared", "france", "tenant_2"] {
        mkdir(&fs, &format!("database/{group}")).await;
        for entry in std::fs::read_dir(fixture.join(group)).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let text = std::fs::read_to_string(entry.path()).unwrap();
            write(&fs, &format!("database/{group}/{name}"), &text).await;
        }
    }
    let scanned =
        MigrationLayout::scan(&fs, "database", MigrationLayoutConfig::default(), config())
            .await
            .unwrap();
    let embedded = EMBEDDED_LAYOUT.layout(config());

    for database in ["france", "tenant_2", "nobody"] {
        let scanned_runner = scanned.runner_for(database);
        let embedded_runner = embedded.runner_for(database);
        assert_eq!(embedded_runner.groups(), scanned_runner.groups());
        let embedded_plan = embedded_runner.plan(&fs).await.unwrap();
        let scanned_plan = scanned_runner.plan(&fs).await.unwrap();
        assert_eq!(fingerprint(&embedded_plan), fingerprint(&scanned_plan));
    }
}
