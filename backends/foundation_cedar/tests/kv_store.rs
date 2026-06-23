#![cfg(feature = "db")]

use foundation_cedar::{KvPolicyStore, PolicyStore};
use foundation_db::traits::KeyValueStore;
use foundation_db::MemoryStorage;

fn seed_store(store: &MemoryStorage, ns: &str) {
    let schema = "entity User;".to_string();
    let policies = "permit(principal, action, resource);".to_string();
    let version = "42".to_string();

    let _ = store.set(&format!("{ns}:schema"), schema);
    let _ = store.set(&format!("{ns}:policies"), policies);
    let _ = store.set(&format!("{ns}:version"), version);
}

#[test]
fn test_kv_store_loads_all() {
    let mem = MemoryStorage::new();
    seed_store(&mem, "authz");

    let store = KvPolicyStore::new(&mem, "authz");
    assert!(store.load_schema().unwrap().contains("entity User"));
    assert!(store.load_policies().unwrap().contains("permit"));
    assert!(store.load_entities().unwrap().is_none());
    assert_eq!(store.version().unwrap(), "42");
}

#[test]
fn test_kv_store_with_entities() {
    let mem = MemoryStorage::new();
    seed_store(&mem, "authz");
    let _ = mem.set("authz:entities", "[]".to_string());

    let store = KvPolicyStore::new(&mem, "authz");
    assert_eq!(store.load_entities().unwrap().unwrap(), "[]");
}

#[test]
fn test_kv_store_missing_key() {
    let mem = MemoryStorage::new();
    let store = KvPolicyStore::new(&mem, "missing");
    assert!(store.load_policies().is_err());
}

#[test]
fn test_kv_store_namespacing() {
    let mem = MemoryStorage::new();
    seed_store(&mem, "ns1");

    let _ = mem.set("ns2:schema", "entity Admin;".to_string());
    let _ = mem.set(
        "ns2:policies",
        "forbid(principal, action, resource);".to_string(),
    );
    let _ = mem.set("ns2:version", "1".to_string());

    let s1 = KvPolicyStore::new(&mem, "ns1");
    let s2 = KvPolicyStore::new(&mem, "ns2");

    assert!(s1.load_schema().unwrap().contains("User"));
    assert!(s2.load_schema().unwrap().contains("Admin"));
}
