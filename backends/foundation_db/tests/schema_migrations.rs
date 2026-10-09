use foundation_db::MIGRATIONS;

#[test]
fn test_migrations_defined() {
    assert!(!MIGRATIONS.is_empty());
    assert_eq!(MIGRATIONS.len(), 24);
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
