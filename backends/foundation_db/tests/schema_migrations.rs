use foundation_db::MIGRATIONS;

#[test]
fn test_migrations_defined() {
    assert!(!MIGRATIONS.is_empty());
    assert_eq!(MIGRATIONS.len(), 21);
}

#[test]
fn test_migration_ids_unique() {
    let ids: Vec<&str> = MIGRATIONS.iter().map(|m| m.id).collect();
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique_ids.len(), "Migration IDs must be unique");
}
