//! State store integration tests.
//!
//! WHY: Verify that the `FileStateStore` backend correctly implements the
//! `StateStore` trait, and that types/helpers work as expected.
//!
//! WHAT: Tests for `FileStateStore` CRUD, `collect_first`, `collect_all`,
//! `drive_to_completion`, `config_hash`, `StateStatus`, and `ResourceState`.
//!
//! HOW: Uses `tempfile::TempDir` for isolated file system tests.

use chrono::Utc;
use serde_json::json;
use tempfile::TempDir;

use foundation_db::state::{
    collect_all, collect_first, config_hash, drive_to_completion, FileStateStore, ResourceState,
    StateStatus, StateStore,
};

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

#[test]
fn file_store_init_creates_directory() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "cloudflare", "staging");
    store.init().unwrap();
    assert!(tmp.path().join(".deployment/cloudflare/staging").exists());
}

#[test]
fn file_store_set_and_get() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let state = make_state("my-worker", StateStatus::Created);
    drive_to_completion(store.set("my-worker", &state).unwrap()).unwrap();

    let retrieved = collect_first(store.get("my-worker").unwrap())
        .unwrap()
        .unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, "my-worker");
    assert_eq!(retrieved.kind, "test::resource");
}

#[test]
fn file_store_get_nonexistent_returns_none() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let result = collect_first(store.get("does-not-exist").unwrap())
        .unwrap()
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn file_store_list_and_count() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let s1 = make_state("alpha", StateStatus::Created);
    let s2 = make_state("bravo", StateStatus::Creating);
    drive_to_completion(store.set("alpha", &s1).unwrap()).unwrap();
    drive_to_completion(store.set("bravo", &s2).unwrap()).unwrap();

    let ids = collect_all(store.list().unwrap()).unwrap();
    assert_eq!(ids, vec!["alpha", "bravo"]);

    let count = collect_first(store.count().unwrap()).unwrap().unwrap();
    assert_eq!(count, 2);
}

#[test]
fn file_store_all() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let s1 = make_state("alpha", StateStatus::Created);
    let s2 = make_state("bravo", StateStatus::Created);
    drive_to_completion(store.set("alpha", &s1).unwrap()).unwrap();
    drive_to_completion(store.set("bravo", &s2).unwrap()).unwrap();

    let all = collect_all(store.all().unwrap()).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, "alpha");
    assert_eq!(all[1].id, "bravo");
}

#[test]
fn file_store_get_batch() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let s1 = make_state("a", StateStatus::Created);
    let s2 = make_state("b", StateStatus::Created);
    let s3 = make_state("c", StateStatus::Created);
    drive_to_completion(store.set("a", &s1).unwrap()).unwrap();
    drive_to_completion(store.set("b", &s2).unwrap()).unwrap();
    drive_to_completion(store.set("c", &s3).unwrap()).unwrap();

    let batch = collect_all(store.get_batch(&["a", "c"]).unwrap()).unwrap();
    assert_eq!(batch.len(), 2);
    let ids: Vec<&str> = batch.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"c"));
}

#[test]
fn file_store_delete() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let state = make_state("doomed", StateStatus::Created);
    drive_to_completion(store.set("doomed", &state).unwrap()).unwrap();
    assert!(collect_first(store.get("doomed").unwrap())
        .unwrap()
        .unwrap()
        .is_some());

    drive_to_completion(store.delete("doomed").unwrap()).unwrap();
    assert!(collect_first(store.get("doomed").unwrap())
        .unwrap()
        .unwrap()
        .is_none());
}

#[test]
fn file_store_delete_nonexistent_is_ok() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    drive_to_completion(store.delete("ghost").unwrap()).unwrap();
}

#[test]
fn file_store_upsert_overwrites() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let v1 = make_state("worker", StateStatus::Creating);
    drive_to_completion(store.set("worker", &v1).unwrap()).unwrap();

    let v2 = make_state("worker", StateStatus::Created);
    drive_to_completion(store.set("worker", &v2).unwrap()).unwrap();

    let retrieved = collect_first(store.get("worker").unwrap())
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.status, StateStatus::Created);
}

#[test]
fn file_store_slash_in_resource_id() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let state = make_state("ns/my-worker", StateStatus::Created);
    drive_to_completion(store.set("ns/my-worker", &state).unwrap()).unwrap();

    let ids = collect_all(store.list().unwrap()).unwrap();
    assert_eq!(ids, vec!["ns/my-worker"]);

    let retrieved = collect_first(store.get("ns/my-worker").unwrap())
        .unwrap()
        .unwrap()
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

#[test]
fn resource_state_needs_deploy() {
    let hash = config_hash(&json!({"key": "worker"})).unwrap();
    let diff = config_hash(&json!({"key": "changed"})).unwrap();

    let created = make_state("worker", StateStatus::Created);
    assert!(!created.needs_deploy(&hash), "same config = no deploy");
    assert!(created.needs_deploy(&diff), "different config = deploy");

    let failed = make_state("worker", StateStatus::Failed {
        error: "err".to_string(),
    });
    assert!(failed.needs_deploy(&hash), "failed always deploys");

    let creating = make_state("worker", StateStatus::Creating);
    assert!(creating.needs_deploy(&hash), "creating always deploys");
}

// ── Helpers ──────────────────────────────────────────────────────────

#[test]
fn collect_first_empty_stream() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    // Empty store — list returns empty stream
    let result = collect_first(store.list().unwrap()).unwrap();
    assert!(result.is_none());
}

#[test]
fn collect_all_empty_stream() {
    let tmp = TempDir::new().unwrap();
    let store = FileStateStore::new(tmp.path(), "test", "dev");
    store.init().unwrap();

    let result = collect_all(store.list().unwrap()).unwrap();
    assert!(result.is_empty());
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
