#![cfg(feature = "db")]

use foundation_cedar::{KvPolicyStore, PolicyStore};
use foundation_db::traits::KeyValueStore;
use foundation_db::{Blocking, MemoryStorage};

/// `KvPolicyStore` reads a sync `KeyValueStore`; the in-memory store reaches
/// it through `Blocking`.
fn memory_store() -> Blocking<MemoryStorage> {
    Blocking::new(MemoryStorage::new())
}

fn seed_store(store: &Blocking<MemoryStorage>, ns: &str) {
    let schema = "entity User;".to_string();
    let policies = "permit(principal, action, resource);".to_string();
    let version = "42".to_string();

    store
        .set(&format!("{ns}:schema"), schema)
        .expect("seed the store");
    store
        .set(&format!("{ns}:policies"), policies)
        .expect("seed the store");
    store
        .set(&format!("{ns}:version"), version)
        .expect("seed the store");
}

#[test]
fn test_kv_store_loads_all() {
    let mem = memory_store();
    seed_store(&mem, "authz");

    let store = KvPolicyStore::new(&mem, "authz");
    assert!(store.load_schema().unwrap().contains("entity User"));
    assert!(store.load_policies().unwrap().contains("permit"));
    assert!(store.load_entities().unwrap().is_none());
    assert_eq!(store.version().unwrap(), "42");
}

#[test]
fn test_kv_store_with_entities() {
    let mem = memory_store();
    seed_store(&mem, "authz");
    mem.set("authz:entities", "[]".to_string())
        .expect("seed the store");

    let store = KvPolicyStore::new(&mem, "authz");
    assert_eq!(store.load_entities().unwrap().unwrap(), "[]");
}

#[test]
fn test_kv_store_missing_key() {
    let mem = memory_store();
    let store = KvPolicyStore::new(&mem, "missing");
    assert!(store.load_policies().is_err());
}

#[test]
fn test_kv_store_namespacing() {
    let mem = memory_store();
    seed_store(&mem, "ns1");

    mem.set("ns2:schema", "entity Admin;".to_string())
        .expect("seed the store");
    mem.set(
        "ns2:policies",
        "forbid(principal, action, resource);".to_string(),
    )
    .expect("seed the store");
    mem.set("ns2:version", "1".to_string())
        .expect("seed the store");

    let s1 = KvPolicyStore::new(&mem, "ns1");
    let s2 = KvPolicyStore::new(&mem, "ns2");

    assert!(s1.load_schema().unwrap().contains("User"));
    assert!(s2.load_schema().unwrap().contains("Admin"));
}
