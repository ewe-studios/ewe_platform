#![cfg(feature = "db")]

use foundation_cedar::{KvPolicyStore, PolicyStore};
use foundation_core::valtron::valtron_test;
use foundation_db::{AsyncKeyValueStore, MemoryStorage};

fn memory_store() -> MemoryStorage {
    MemoryStorage::new()
}

async fn seed_store(store: &MemoryStorage, ns: &str) {
    let schema = "entity User;".to_string();
    let policies = "permit(principal, action, resource);".to_string();
    let version = "42".to_string();

    store
        .set_async(&format!("{ns}:schema"), schema)
        .await
        .expect("seed the store");
    store
        .set_async(&format!("{ns}:policies"), policies)
        .await
        .expect("seed the store");
    store
        .set_async(&format!("{ns}:version"), version)
        .await
        .expect("seed the store");
}

#[valtron_test]
async fn test_kv_store_loads_all() {
    let mem = memory_store();
    seed_store(&mem, "authz").await;

    let store = KvPolicyStore::new(&mem, "authz");
    assert!(store.load_schema().await.unwrap().contains("entity User"));
    assert!(store.load_policies().await.unwrap().contains("permit"));
    assert!(store.load_entities().await.unwrap().is_none());
    assert_eq!(store.version().await.unwrap(), "42");
}

#[valtron_test]
async fn test_kv_store_with_entities() {
    let mem = memory_store();
    seed_store(&mem, "authz").await;
    mem.set_async("authz:entities", "[]".to_string())
        .await
        .expect("seed the store");

    let store = KvPolicyStore::new(&mem, "authz");
    assert_eq!(store.load_entities().await.unwrap().unwrap(), "[]");
}

#[valtron_test]
async fn test_kv_store_missing_key() {
    let mem = memory_store();
    let store = KvPolicyStore::new(&mem, "missing");
    assert!(store.load_policies().await.is_err());
}

#[valtron_test]
async fn test_kv_store_namespacing() {
    let mem = memory_store();
    seed_store(&mem, "ns1").await;

    mem.set_async("ns2:schema", "entity Admin;".to_string())
        .await
        .expect("seed the store");
    mem.set_async(
        "ns2:policies",
        "forbid(principal, action, resource);".to_string(),
    )
    .await
    .expect("seed the store");
    mem.set_async("ns2:version", "1".to_string())
        .await
        .expect("seed the store");

    let s1 = KvPolicyStore::new(&mem, "ns1");
    let s2 = KvPolicyStore::new(&mem, "ns2");

    assert!(s1.load_schema().await.unwrap().contains("User"));
    assert!(s2.load_schema().await.unwrap().contains("Admin"));
}
