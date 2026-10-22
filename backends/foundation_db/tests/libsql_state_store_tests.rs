//! `LibsqlStore` as a `StateStore` on a local database: CRUD, the SQL
//! prefix overrides, and the `find_by_*` queries against the stored columns.

#![cfg(feature = "libsql")]

use chrono::Utc;
use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStoreStream;
use foundation_db::state::{ResourceState, StateStatus, StateStore};
use foundation_db::LibsqlStore;
use futures_lite::StreamExt;
use serde_json::json;

fn state(id: &str, kind: &str, status: StateStatus) -> ResourceState {
    let now = Utc::now();
    ResourceState {
        id: id.to_string(),
        kind: kind.to_string(),
        provider: "test".to_string(),
        status,
        environment: None,
        config_hash: String::new(),
        output: json!({ "id": id }),
        config_snapshot: json!(null),
        created_at: now,
        updated_at: now,
    }
}

async fn collect<T>(stream: StateStoreStream<T>) -> Vec<T> {
    stream.map(Result::unwrap).collect().await
}

fn ids(states: &[ResourceState]) -> Vec<&str> {
    states.iter().map(|s| s.id.as_str()).collect()
}

/// A local state store holding five resources, two of them failed.
async fn seeded() -> (LibsqlStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db").to_string_lossy().into_owned();
    let store = LibsqlStore::new_state(&path, "my-proj", "dev").await.unwrap();
    store.init().await.unwrap();
    let failed = || StateStatus::Failed {
        error: "boom: quota".to_string(),
    };
    for (id, kind, status) in [
        ("a", "worker", StateStatus::Created),
        ("b", "bucket", StateStatus::Creating),
        ("ns/c", "worker", failed()),
        ("ns/d", "bucket", StateStatus::Created),
        ("ns/e", "worker", failed()),
    ] {
        store.set(id, &state(id, kind, status)).await.unwrap();
    }
    (store, dir)
}

#[valtron_test]
async fn set_get_and_upsert() {
    let (store, _dir) = seeded().await;

    let got = store.get("ns/d").await.unwrap().unwrap();
    assert_eq!(got.kind, "bucket");
    assert_eq!(got.status, StateStatus::Created);

    store
        .set("ns/d", &state("ns/d", "bucket", StateStatus::Deleting))
        .await
        .unwrap();
    assert_eq!(store.get("ns/d").await.unwrap().unwrap().status, StateStatus::Deleting);
    assert!(store.get("missing").await.unwrap().is_none());
}

#[valtron_test]
async fn list_count_and_prefixes() {
    let (store, _dir) = seeded().await;

    assert_eq!(collect(store.list().await.unwrap()).await, vec!["a", "b", "ns/c", "ns/d", "ns/e"]);
    assert_eq!(store.count().await.unwrap(), 5);
    assert_eq!(collect(store.list_by_prefix("ns/").await.unwrap()).await, vec!["ns/c", "ns/d", "ns/e"]);
    assert_eq!(store.count_by_prefix("ns/").await.unwrap(), 3);

    let in_ns = collect(store.all_by_prefix("ns/").await.unwrap()).await;
    assert_eq!(ids(&in_ns), vec!["ns/c", "ns/d", "ns/e"]);

    let batch = collect(store.get_batch(&["b", "missing", "ns/e"]).await.unwrap()).await;
    assert_eq!(ids(&batch), vec!["b", "ns/e"]);

    assert_eq!(store.delete_by_prefix("ns/").await.unwrap(), 3);
    assert_eq!(collect(store.list().await.unwrap()).await, vec!["a", "b"]);
}

#[valtron_test]
async fn find_by_status_matches_the_display_form() {
    let (store, _dir) = seeded().await;

    let created = collect(store.find_by_status(&StateStatus::Created.to_string()).await.unwrap()).await;
    assert_eq!(ids(&created), vec!["a", "ns/d"]);

    let failed_status = StateStatus::Failed {
        error: "boom: quota".to_string(),
    };
    let failed = collect(store.find_by_status(&failed_status.to_string()).await.unwrap()).await;
    assert_eq!(ids(&failed), vec!["ns/c", "ns/e"]);

    assert!(collect(store.find_by_status("deleted").await.unwrap()).await.is_empty());
}

#[valtron_test]
async fn find_by_kind_and_provider() {
    let (store, _dir) = seeded().await;

    let workers = collect(store.find_by_kind("worker").await.unwrap()).await;
    assert_eq!(ids(&workers), vec!["a", "ns/c", "ns/e"]);
    assert_eq!(collect(store.find_by_provider("test").await.unwrap()).await.len(), 5);
    assert!(collect(store.find_by_provider("other").await.unwrap()).await.is_empty());
}

#[valtron_test]
async fn delete_removes_and_missing_is_ok() {
    let (store, _dir) = seeded().await;

    store.delete("a").await.unwrap();
    store.delete("never-there").await.unwrap();
    assert!(store.get("a").await.unwrap().is_none());
    assert_eq!(store.count().await.unwrap(), 4);
}
