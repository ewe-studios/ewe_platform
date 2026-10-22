//! State store integration tests.
//!
//! WHY: Verify that the `FileStateStore` backend correctly implements the
//! `StateStore` trait, and that types/helpers work as expected.
//!
//! WHAT: Tests for `FileStateStore` CRUD, `config_hash`, `StateStatus`, and
//! `ResourceState`.
//!
//! HOW: Uses `tempfile::TempDir` for isolated file system tests.

use chrono::Utc;
use serde_json::json;
use tempfile::TempDir;

use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStoreStream;
use foundation_db::state::{config_hash, FileStateStore, ResourceState, StateStatus, StateStore};
use futures_lite::StreamExt;

/// Drain a state stream, failing the test on any item error.
async fn collect<T>(stream: StateStoreStream<T>) -> Vec<T> {
    stream.map(Result::unwrap).collect().await
}

/// Helper: create a test `ResourceState`.
fn make_state(id: &str, status: StateStatus) -> ResourceState {
    let now = Utc::now();
    ResourceState {
        id: id.to_string(),
        kind: "test::resource".to_string(),
        provider: "test".to_string(),
        status,
        environment: Some("staging".to_string()),
        config_hash: config_hash(&json!({"key": id})).unwrap(),
        output: json!({"url": format!("https://example.com/{id}")}),
        config_snapshot: json!({"key": id}),
        created_at: now,
        updated_at: now,
    }
}

// ── FileStateStore CRUD ──────────────────────────────────────────────

#[valtron_test]
async fn file_store_init_creates_directory() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "staging");
    store.init().await.unwrap();
    assert!(tmp.path().join(".deployment/test-project/staging").exists());
}

#[valtron_test]
async fn file_store_set_and_get() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let state = make_state("my-worker", StateStatus::Created);
    store.set("my-worker", &state).await.unwrap();

    let retrieved = store.get("my-worker").await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, "my-worker");
    assert_eq!(retrieved.kind, "test::resource");
}

#[valtron_test]
async fn file_store_get_nonexistent_returns_none() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let result = store.get("does-not-exist").await.unwrap();
    assert!(result.is_none());
}

#[valtron_test]
async fn file_store_list_and_count() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let s1 = make_state("alpha", StateStatus::Created);
    let s2 = make_state("bravo", StateStatus::Creating);
    store.set("alpha", &s1).await.unwrap();
    store.set("bravo", &s2).await.unwrap();

    let ids = collect(store.list().await.unwrap()).await;
    assert_eq!(ids, vec!["alpha", "bravo"]);

    let count = store.count().await.unwrap();
    assert_eq!(count, 2);
}

#[valtron_test]
async fn file_store_all() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let s1 = make_state("alpha", StateStatus::Created);
    let s2 = make_state("bravo", StateStatus::Created);
    store.set("alpha", &s1).await.unwrap();
    store.set("bravo", &s2).await.unwrap();

    let all = collect(store.all().await.unwrap()).await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, "alpha");
    assert_eq!(all[1].id, "bravo");
}

#[valtron_test]
async fn file_store_get_batch() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let s1 = make_state("a", StateStatus::Created);
    let s2 = make_state("b", StateStatus::Created);
    let s3 = make_state("c", StateStatus::Created);
    store.set("a", &s1).await.unwrap();
    store.set("b", &s2).await.unwrap();
    store.set("c", &s3).await.unwrap();

    let batch = collect(store.get_batch(&["a", "c"]).await.unwrap()).await;
    assert_eq!(batch.len(), 2);
    let ids: Vec<&str> = batch.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"c"));
}

#[valtron_test]
async fn file_store_delete() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let state = make_state("doomed", StateStatus::Created);
    store.set("doomed", &state).await.unwrap();
    assert!(store.get("doomed").await.unwrap()
        .is_some());

    store.delete("doomed").await.unwrap();
    assert!(store.get("doomed").await.unwrap()
        .is_none());
}

#[valtron_test]
async fn file_store_delete_nonexistent_is_ok() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    store.delete("ghost").await.unwrap();
}

#[valtron_test]
async fn file_store_upsert_overwrites() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let v1 = make_state("worker", StateStatus::Creating);
    store.set("worker", &v1).await.unwrap();

    let v2 = make_state("worker", StateStatus::Created);
    store.set("worker", &v2).await.unwrap();

    let retrieved = store.get("worker").await.unwrap()
        .unwrap();
    assert_eq!(retrieved.status, StateStatus::Created);
}

#[valtron_test]
async fn file_store_slash_in_resource_id() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let state = make_state("ns/my-worker", StateStatus::Created);
    store.set("ns/my-worker", &state).await.unwrap();

    let ids = collect(store.list().await.unwrap()).await;
    assert_eq!(ids, vec!["ns/my-worker"]);

    let retrieved = store.get("ns/my-worker").await.unwrap()
        .unwrap();
    assert_eq!(retrieved.id, "ns/my-worker");
}

// ── StateStatus & ResourceState ──────────────────────────────────────

#[test]
fn state_status_serialization_roundtrip() {
    let statuses = vec![
        StateStatus::Creating,
        StateStatus::Created,
        StateStatus::Updating,
        StateStatus::Deleting,
        StateStatus::Deleted,
        StateStatus::Failed {
            error: "out of quota".to_string(),
        },
    ];

    for status in &statuses {
        let json = serde_json::to_string(status).unwrap();
        let deserialized: StateStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, &deserialized);
    }
}

#[test]
fn state_status_display() {
    assert_eq!(StateStatus::Creating.to_string(), "creating");
    assert_eq!(StateStatus::Created.to_string(), "created");
    assert_eq!(
        StateStatus::Failed {
            error: "boom".to_string()
        }
        .to_string(),
        "failed: boom"
    );
}

#[test]
fn resource_state_config_changed() {
    let state = make_state("worker", StateStatus::Created);
    let same_hash = config_hash(&json!({"key": "worker"})).unwrap();
    let diff_hash = config_hash(&json!({"key": "different"})).unwrap();

    assert!(!state.config_changed(&same_hash));
    assert!(state.config_changed(&diff_hash));
}

#[valtron_test]
async fn resource_state_needs_deploy() {
    let hash = config_hash(&json!({"key": "worker"})).unwrap();
    let diff = config_hash(&json!({"key": "changed"})).unwrap();

    let created = make_state("worker", StateStatus::Created);
    assert!(!created.needs_deploy(&hash), "same config = no deploy");
    assert!(created.needs_deploy(&diff), "different config = deploy");

    let failed = make_state(
        "worker",
        StateStatus::Failed {
            error: "err".to_string(),
        },
    );
    assert!(failed.needs_deploy(&hash), "failed always deploys");

    let creating = make_state("worker", StateStatus::Creating);
    assert!(creating.needs_deploy(&hash), "creating always deploys");
}

// ── Empty store ──────────────────────────────────────────────────────────

#[valtron_test]
async fn empty_store_counts_zero() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    assert_eq!(store.count().await.unwrap(), 0);
}

#[valtron_test]
async fn empty_store_lists_nothing() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test-project", "dev");
    store.init().await.unwrap();

    let result = collect(store.list().await.unwrap()).await;
    assert_eq!(result, Vec::<String>::new());
}

// ── Config hash ──────────────────────────────────────────────────────

#[test]
fn config_hash_deterministic() {
    let config = json!({"name": "my-worker", "memory": 128});
    let h1 = config_hash(&config).unwrap();
    let h2 = config_hash(&config).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn config_hash_different_values() {
    let a = json!({"name": "a"});
    let b = json!({"name": "b"});
    assert_ne!(config_hash(&a).unwrap(), config_hash(&b).unwrap());
}
