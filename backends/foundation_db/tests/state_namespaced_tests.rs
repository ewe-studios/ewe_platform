//! `NamespacedStore` tests: every operation stays inside its namespace.

use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStoreStream;
use foundation_db::state::namespaced::NamespacedStore;
use foundation_db::state::{FileStateStore, ResourceState, StateStatus, StateStore};
use futures_lite::StreamExt;

async fn temp_store() -> (tempfile::TempDir, Arc<FileStateStore>) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    store.init().await.expect("Failed to init store");
    (temp_dir, Arc::new(store))
}

/// Drain a state stream, failing the test on any item error.
async fn collect<T>(stream: StateStoreStream<T>) -> Vec<T> {
    stream.map(Result::unwrap).collect().await
}

/// Write a state outside any namespace, straight into the inner store.
async fn set_unrelated(store: &FileStateStore, id: &str) {
    let now = chrono::Utc::now();
    let state = ResourceState {
        id: id.to_string(),
        kind: String::new(),
        provider: String::new(),
        status: StateStatus::Created,
        environment: None,
        config_hash: String::new(),
        output: serde_json::json!(0),
        config_snapshot: serde_json::json!(null),
        created_at: now,
        updated_at: now,
    };
    store.set(id, &state).await.unwrap();
}

#[valtron_test]
async fn test_prefix_isolation() {
    let (_temp, store) = temp_store().await;

    let ns_a = NamespacedStore::new(Arc::clone(&store), "namespace-a");
    let ns_b = NamespacedStore::new(store, "namespace-b");

    ns_a.store_typed("key1", &"value-a").await.unwrap();
    ns_b.store_typed("key1", &"value-b").await.unwrap();

    let a_val: Option<String> = ns_a.get_typed("key1").await.unwrap();
    let b_val: Option<String> = ns_b.get_typed("key1").await.unwrap();

    assert_eq!(a_val, Some("value-a".to_string()));
    assert_eq!(b_val, Some("value-b".to_string()));
}

#[valtron_test]
async fn test_list_filters_by_prefix() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "test");

    ns.store_typed("resource-1", &1).await.unwrap();
    ns.store_typed("resource-2", &2).await.unwrap();
    set_unrelated(&store, "other-key").await;

    let ids = collect(ns.list().await.unwrap()).await;
    assert_eq!(ids, vec!["resource-1".to_string(), "resource-2".to_string()]);
}

#[valtron_test]
async fn test_delete_removes_key() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(store, "del-test");

    ns.store_typed("to-delete", &42).await.unwrap();
    assert!(ns.get_typed::<i32>("to-delete").await.unwrap().is_some());

    ns.remove("to-delete").await.unwrap();
    assert!(ns.get_typed::<i32>("to-delete").await.unwrap().is_none());
}

#[valtron_test]
async fn test_struct_serialization() {
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestData {
        name: String,
        count: u32,
    }

    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(store, "struct-test");

    let data = TestData {
        name: "test".to_string(),
        count: 42,
    };
    ns.store_typed("data", &data).await.unwrap();

    let loaded: Option<TestData> = ns.get_typed("data").await.unwrap();
    assert_eq!(loaded, Some(data));
}

#[valtron_test]
async fn test_all_returns_only_namespaced() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "all-test");
    ns.store_typed("one", &1).await.unwrap();
    ns.store_typed("two", &2).await.unwrap();
    set_unrelated(&store, "unrelated").await;

    let all = collect(ns.all().await.unwrap()).await;
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.id.ends_with("one")));
    assert!(all.iter().any(|s| s.id.ends_with("two")));
    assert!(!all.iter().any(|s| s.id == "unrelated"));
}

#[valtron_test]
async fn test_count_only_namespaced() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "count-test");

    ns.store_typed("a", &1).await.unwrap();
    ns.store_typed("b", &2).await.unwrap();
    set_unrelated(&store, "other").await;

    assert_eq!(ns.count().await.unwrap(), 2);
}

#[valtron_test]
async fn test_list_by_prefix_delegates() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(store, "prefix-test");

    ns.store_typed("alpha-1", &1).await.unwrap();
    ns.store_typed("alpha-2", &2).await.unwrap();
    ns.store_typed("beta-1", &3).await.unwrap();

    let alpha_ids = collect(ns.list_by_prefix("alpha-").await.unwrap()).await;
    assert_eq!(alpha_ids, vec!["alpha-1".to_string(), "alpha-2".to_string()]);
}

#[valtron_test]
async fn test_count_by_prefix_delegates() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "count-prefix");

    ns.store_typed("a", &1).await.unwrap();
    ns.store_typed("b", &2).await.unwrap();
    set_unrelated(&store, "other").await;

    assert_eq!(ns.count_by_prefix("").await.unwrap(), 2);
}

#[valtron_test]
async fn test_delete_by_prefix_deletes_only_namespaced() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "del-prefix");

    ns.store_typed("keep", &1).await.unwrap();
    ns.store_typed("remove-1", &2).await.unwrap();
    ns.store_typed("remove-2", &3).await.unwrap();
    set_unrelated(&store, "remove-unrelated").await;

    assert_eq!(ns.delete_by_prefix("remove-").await.unwrap(), 2);

    assert!(ns.get_typed::<i32>("keep").await.unwrap().is_some());
    assert!(ns.get_typed::<i32>("remove-1").await.unwrap().is_none());
    assert!(store.get("remove-unrelated").await.unwrap().is_some());
}

#[valtron_test]
async fn test_all_by_prefix_delegates() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "all-prefix");

    ns.store_typed("one", &10).await.unwrap();
    ns.store_typed("two", &20).await.unwrap();
    set_unrelated(&store, "unrelated").await;

    let all = collect(ns.all_by_prefix("").await.unwrap()).await;
    assert_eq!(all.len(), 2);
    assert!(!all.iter().any(|s| s.id == "unrelated"));
}

#[valtron_test]
async fn test_set_writes_inside_the_namespace() {
    let (_temp, store) = temp_store().await;
    let ns = NamespacedStore::new(Arc::clone(&store), "set-ns");

    let now = chrono::Utc::now();
    let state = ResourceState {
        id: "web".to_string(),
        kind: "worker".to_string(),
        provider: "test".to_string(),
        status: StateStatus::Created,
        environment: None,
        config_hash: String::new(),
        output: serde_json::json!(1),
        config_snapshot: serde_json::json!(null),
        created_at: now,
        updated_at: now,
    };
    ns.set("web", &state).await.unwrap();

    assert!(store.get("web").await.unwrap().is_none(), "wrote outside the namespace");
    assert_eq!(ns.get("web").await.unwrap().unwrap().kind, "worker");
    assert_eq!(collect(ns.list().await.unwrap()).await, vec!["web".to_string()]);
}
