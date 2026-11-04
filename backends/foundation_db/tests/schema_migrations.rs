use foundation_db::MIGRATIONS;

#[test]
fn test_migrations_defined() {
    assert!(!MIGRATIONS.is_empty());
    assert_eq!(MIGRATIONS.len(), 27);
}

#[test]
fn test_social_login_migrations_present() {
    let ids: Vec<&str> = MIGRATIONS.iter().map(|m| m.id).collect();
    assert!(
        ids.contains(&"024_create_upstream_providers"),
        "024_create_upstream_providers must be registered"
    );
    assert!(
        ids.contains(&"025_create_user_provider_links"),
        "025_create_user_provider_links must be registered"
    );
}

#[test]
fn test_migration_ids_unique() {
    let ids: Vec<&str> = MIGRATIONS.iter().map(|m| m.id).collect();
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique_ids.len(), "Migration IDs must be unique");
}

/// Every SQL file in `src/core/schema/sql` is a registered migration, and the
/// other way round. (A file nobody runs, like the old `023_create_vectors.sql`,
/// is dead weight and hides mistakes.)
#[test]
fn every_migration_file_is_registered_and_every_migration_has_a_file() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/schema/sql");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.to_ascii_lowercase().ends_with(".sql"))
        .map(|name| name.trim_end_matches(".sql").to_string())
        .collect();
    files.sort();
    let mut ids: Vec<String> = MIGRATIONS.iter().map(|m| m.id.to_string()).collect();
    ids.sort();
    assert_eq!(files, ids, "SQL files and registered migration ids differ");
}
