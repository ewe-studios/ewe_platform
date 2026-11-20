//! Every Structured Query Language (SQL) file is in `DB_CORE`.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_db::{MigrationSource, DB_CORE, DB_CORE_GROUP, MIGRATIONS};

#[test]
fn every_sql_file_is_a_core_migration() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/schema/sql");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".sql"))
        .map(|name| name.trim_end_matches(".sql").to_string())
        .collect();
    files.sort();
    let mut ids: Vec<String> = MIGRATIONS
        .iter()
        .map(|migration| migration.id.to_string())
        .collect();
    ids.sort();
    assert_eq!(
        files, ids,
        "SQL files and core migration identifiers differ"
    );
}

#[test]
fn core_numbers_are_one_to_twenty_eight() {
    let numbers: Vec<u64> = MIGRATIONS
        .iter()
        .map(|migration| migration.id.split('_').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(numbers, (1..=28).collect::<Vec<u64>>());
}

#[test]
fn db_core_is_the_static_core_group() {
    assert_eq!(DB_CORE.group, DB_CORE_GROUP);
    match &DB_CORE.source {
        MigrationSource::Static(migrations) => {
            assert_eq!(migrations.len(), MIGRATIONS.len());
            assert!(migrations
                .iter()
                .any(|migration| migration.id == "023_create_vectors"));
        }
        other => panic!("DB_CORE must be a Static group, got {other:?}"),
    }
}
