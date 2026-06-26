//! Tests extracted from core/state/namespaced.rs

use std::sync::Arc;

use foundation_core::valtron::ThreadedValue;
use foundation_db::state::namespaced::NamespacedStore;
use foundation_db::state::{
    drive_to_completion, FileStateStore, ResourceState, StateStatus, StateStore,
};

fn temp_store() -> (tempfile::TempDir, FileStateStore) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let store = FileStateStore::with_root(temp_dir.path().to_path_buf());
    store.init().expect("Failed to init store");
    (temp_dir, store)
}

#[test]
fn test_prefix_isolation() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);

    let ns_a = NamespacedStore::new(Arc::clone(&store_arc), "namespace-a");
    let ns_b = NamespacedStore::new(store_arc, "namespace-b");

    ns_a.store_typed("key1", &"value-a").unwrap();
    ns_b.store_typed("key1", &"value-b").unwrap();

    let a_val: Option<String> = ns_a.get_typed("key1").unwrap();
    let b_val: Option<String> = ns_b.get_typed("key1").unwrap();

    assert_eq!(a_val, Some("value-a".to_string()));
    assert_eq!(b_val, Some("value-b".to_string()));
}

#[test]
fn test_list_filters_by_prefix() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "test");

    ns.store_typed("resource-1", &1).unwrap();
    ns.store_typed("resource-2", &2).unwrap();
    drive_to_completion(
        store_arc
            .set(
                "other-key",
                &ResourceState {
                    id: "other-key".to_string(),
                    kind: String::new(),
                    provider: String::new(),
                    status: StateStatus::Created,
                    environment: None,
                    config_hash: String::new(),
                    output: serde_json::json!(99),
                    config_snapshot: serde_json::json!(null),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            )
            .unwrap(),
    )
    .unwrap();

    let namespaced_ids: Vec<_> = ns
        .list()
        .unwrap()
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(id)) => Some(id),
            _ => None,
        })
        .collect();

    assert!(namespaced_ids.contains(&"resource-1".to_string()));
    assert!(namespaced_ids.contains(&"resource-2".to_string()));
    assert!(!namespaced_ids.contains(&"other-key".to_string()));
}

#[test]
fn test_delete_removes_key() {
    let (_temp, store) = temp_store();
    let ns = NamespacedStore::new(Arc::new(store), "del-test");

    ns.store_typed("to-delete", &42).unwrap();
    assert!(ns.get_typed::<i32>("to-delete").unwrap().is_some());

    ns.remove("to-delete").unwrap();
    assert!(ns.get_typed::<i32>("to-delete").unwrap().is_none());
}

#[test]
fn test_struct_serialization() {
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestData {
        name: String,
        count: u32,
    }

    let (_temp, store) = temp_store();
    let ns = NamespacedStore::new(Arc::new(store), "struct-test");

    let data = TestData {
        name: "test".to_string(),
        count: 42,
    };
    ns.store_typed("data", &data).unwrap();

    let loaded: Option<TestData> = ns.get_typed("data").unwrap();
    assert_eq!(loaded, Some(data));
}

#[test]
fn test_all_returns_only_namespaced() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "all-test");
    ns.store_typed("one", &1).unwrap();
    ns.store_typed("two", &2).unwrap();

    drive_to_completion(
        store_arc
            .set(
                "unrelated",
                &ResourceState {
                    id: "unrelated".to_string(),
                    kind: String::new(),
                    provider: String::new(),
                    status: StateStatus::Created,
                    environment: None,
                    config_hash: String::new(),
                    output: serde_json::json!(0),
                    config_snapshot: serde_json::json!(null),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            )
            .unwrap(),
    )
    .unwrap();

    let all: Vec<_> = ns
        .all()
        .unwrap()
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(s)) => Some(s),
            _ => None,
        })
        .collect();

    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.id.ends_with("one")));
    assert!(all.iter().any(|s| s.id.ends_with("two")));
    assert!(!all.iter().any(|s| s.id == "unrelated"));
}

#[test]
fn test_count_only_namespaced() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "count-test");

    ns.store_typed("a", &1).unwrap();
    ns.store_typed("b", &2).unwrap();

    drive_to_completion(
        store_arc
            .set(
                "other",
                &ResourceState {
                    id: "other".to_string(),
                    kind: String::new(),
                    provider: String::new(),
                    status: StateStatus::Created,
                    environment: None,
                    config_hash: String::new(),
                    output: serde_json::json!(0),
                    config_snapshot: serde_json::json!(null),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            )
            .unwrap(),
    )
    .unwrap();

    let count: Vec<_> = ns
        .count()
        .unwrap()
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(c)) => Some(c),
            _ => None,
        })
        .collect();

    assert_eq!(count, vec![2]);
}

#[test]
fn test_list_by_prefix_delegates() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "prefix-test");

    ns.store_typed("alpha-1", &1).unwrap();
    ns.store_typed("alpha-2", &2).unwrap();
    ns.store_typed("beta-1", &3).unwrap();

    let alpha_ids: Vec<_> = ns
        .list_by_prefix("alpha-")
        .unwrap()
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(id)) => Some(id),
            _ => None,
        })
        .collect();

    assert_eq!(alpha_ids.len(), 2);
    assert!(alpha_ids.contains(&"alpha-1".to_string()));
    assert!(alpha_ids.contains(&"alpha-2".to_string()));
}

#[test]
fn test_count_by_prefix_delegates() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "count-prefix");

    ns.store_typed("a", &1).unwrap();
    ns.store_typed("b", &2).unwrap();
    drive_to_completion(
        store_arc
            .set(
                "other",
                &ResourceState {
                    id: "other".to_string(),
                    kind: String::new(),
                    provider: String::new(),
                    status: StateStatus::Created,
                    environment: None,
                    config_hash: String::new(),
                    output: serde_json::json!(0),
                    config_snapshot: serde_json::json!(null),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            )
            .unwrap(),
    )
    .unwrap();

    let count: Vec<_> = ns
        .count_by_prefix("")
        .unwrap()
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(c)) => Some(c),
            _ => None,
        })
        .collect();

    assert_eq!(count, vec![2]);
}

#[test]
fn test_delete_by_prefix_deletes_only_namespaced() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "del-prefix");

    ns.store_typed("keep", &1).unwrap();
    ns.store_typed("remove-1", &2).unwrap();
    ns.store_typed("remove-2", &3).unwrap();

    // Insert non-namespaced key directly
    drive_to_completion(
        store_arc
            .set(
                "unrelated",
                &ResourceState {
                    id: "unrelated".to_string(),
                    kind: String::new(),
                    provider: String::new(),
                    status: StateStatus::Created,
                    environment: None,
                    config_hash: String::new(),
                    output: serde_json::json!(0),
                    config_snapshot: serde_json::json!(null),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            )
            .unwrap(),
    )
    .unwrap();

    ns.remove("remove-1").unwrap();
    ns.remove("remove-2").unwrap();

    assert!(ns.get_typed::<i32>("keep").unwrap().is_some());
    // Verify unrelated is still there
    let stream = store_arc.get("unrelated").unwrap();
    let found: Vec<_> = stream
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(Some(s))) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(found.len(), 1);
}

#[test]
fn test_all_by_prefix_delegates() {
    let (_temp, store) = temp_store();
    let store_arc = Arc::new(store);
    let ns = NamespacedStore::new(Arc::clone(&store_arc), "all-prefix");

    ns.store_typed("one", &10).unwrap();
    ns.store_typed("two", &20).unwrap();

    // Insert non-namespaced key directly
    drive_to_completion(
        store_arc
            .set(
                "unrelated",
                &ResourceState {
                    id: "unrelated".to_string(),
                    kind: String::new(),
                    provider: String::new(),
                    status: StateStatus::Created,
                    environment: None,
                    config_hash: String::new(),
                    output: serde_json::json!(0),
                    config_snapshot: serde_json::json!(null),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
            )
            .unwrap(),
    )
    .unwrap();

    let all: Vec<_> = ns
        .all()
        .unwrap()
        .filter_map(|i| match i {
            ThreadedValue::Value(Ok(s)) => Some(s),
            _ => None,
        })
        .collect();

    assert_eq!(all.len(), 2);
    assert!(!all.iter().any(|s| s.id == "unrelated"));
}
