use std::io::Write;

use foundation_cedar::{FilePolicyStore, PolicyStore};
use foundation_core::valtron::valtron_test;

#[valtron_test]
async fn test_file_store_loads_policies() {
    let dir = std::env::temp_dir().join("cedar_file_store_test");
    let _ = std::fs::create_dir_all(&dir);

    let schema_path = dir.join("schema.cedarschema");
    let policy_path = dir.join("policies.cedar");

    let mut sf = std::fs::File::create(&schema_path).unwrap();
    writeln!(sf, "entity User;").unwrap();

    let mut pf = std::fs::File::create(&policy_path).unwrap();
    writeln!(pf, "permit(principal, action, resource);").unwrap();

    let store = FilePolicyStore::new(&schema_path, &policy_path);
    assert!(store.load_schema().await.unwrap().contains("entity User"));
    assert!(store.load_policies().await.unwrap().contains("permit"));
    assert!(store.load_entities().await.unwrap().is_none());
    assert!(!store.version().await.unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn test_file_store_with_entities() {
    let dir = std::env::temp_dir().join("cedar_file_store_ent_test");
    let _ = std::fs::create_dir_all(&dir);

    let schema_path = dir.join("schema.cedarschema");
    let policy_path = dir.join("policies.cedar");
    let entities_path = dir.join("entities.json");

    std::fs::write(&schema_path, "entity User;").unwrap();
    std::fs::write(&policy_path, "permit(principal, action, resource);").unwrap();
    std::fs::write(&entities_path, "[]").unwrap();

    let store = FilePolicyStore::new(&schema_path, &policy_path).with_entities(&entities_path);
    assert_eq!(store.load_entities().await.unwrap().unwrap(), "[]");

    let _ = std::fs::remove_dir_all(&dir);
}

#[valtron_test]
async fn test_file_store_missing_file() {
    let store = FilePolicyStore::new("/nonexistent/schema", "/nonexistent/policies");
    assert!(store.load_policies().await.is_err());
    assert!(store.load_schema().await.is_err());
}
