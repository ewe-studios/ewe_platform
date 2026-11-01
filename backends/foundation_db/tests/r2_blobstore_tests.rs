//! `BlobStore` integration tests for the R2 backend against a local
//! wrangler worker emulating the Cloudflare R2 REST API.
//!
//! Endpoint configuration lives in [`common`] — set `CF_INTEGRATION_TEST=1`
//! and optionally override `LOCAL_CF_API_BASE` to run these tests.

mod common;

use chrono::Utc;
use common::make_r2_store;
use foundation_core::valtron::valtron_test;
use foundation_db::core::state::traits::StateStoreStream;
use foundation_db::state::{ResourceState, StateStatus, StateStore};
use foundation_db::{BlobStore, R2Store};
use futures_lite::StreamExt;
use serde_json::json;

fn store() -> Option<R2Store> {
    let store = make_r2_store();
    if store.is_none() {
        println!("Skipping R2 test - miniflare not available (set CF_INTEGRATION_TEST=1 and start the worker stub)");
    }
    store
}

fn key(name: &str) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{name}_{secs}")
}

#[valtron_test]
async fn test_r2_blobstore_put_get() {
    let Some(storage) = store() else { return };
    let key = key("test_put_get");

    storage.put_blob(&key, b"Hello, R2!").await.unwrap();
    assert_eq!(storage.get_blob(&key).await.unwrap(), Some(b"Hello, R2!".to_vec()));

    storage.delete_blob(&key).await.unwrap();
}

#[valtron_test]
async fn test_r2_blobstore_delete() {
    let Some(storage) = store() else { return };
    let key = key("test_delete");

    storage.put_blob(&key, b"To be deleted").await.unwrap();
    assert!(storage.blob_exists(&key).await.unwrap());

    storage.delete_blob(&key).await.unwrap();
    assert!(!storage.blob_exists(&key).await.unwrap());
}

#[valtron_test]
async fn test_r2_blobstore_exists() {
    let Some(storage) = store() else { return };
    let key = key("test_exists");

    assert!(!storage.blob_exists(&key).await.unwrap());
    storage.put_blob(&key, b"Exists!").await.unwrap();
    assert!(storage.blob_exists(&key).await.unwrap());

    storage.delete_blob(&key).await.unwrap();
}

#[valtron_test]
async fn test_r2_blobstore_binary_data() {
    let Some(storage) = store() else { return };
    let key = key("test_binary");

    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    storage.put_blob(&key, &test_data).await.unwrap();
    assert_eq!(storage.get_blob(&key).await.unwrap(), Some(test_data));

    storage.delete_blob(&key).await.unwrap();
}

#[valtron_test]
async fn test_r2_blobstore_delete_nonexistent() {
    let Some(storage) = store() else { return };
    // Deleting a missing key is not an error.
    storage.delete_blob(&key("test_del_none")).await.unwrap();
}

// ── R2 object listing (state mode) against the worker stub ──────────

fn state(id: &str) -> ResourceState {
    let now = Utc::now();
    ResourceState {
        id: id.to_string(),
        kind: "worker".to_string(),
        provider: "test".to_string(),
        status: StateStatus::Created,
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

/// A state-mode R2 store pointed at the worker stub, or `None` to skip.
fn state_store(stage: &str) -> Option<R2Store> {
    if !common::is_local_cf_available() {
        println!("Skipping R2 listing test - miniflare not available");
        return None;
    }
    let bucket =
        std::env::var(common::ENV_R2_BUCKET).unwrap_or_else(|_| "test-bucket".to_string());
    Some(R2Store::new_state_with_base_url(
        "test-token",
        "test-account",
        &bucket,
        "proj",
        stage,
        &common::local_cf_api_base(),
    ))
}

#[valtron_test]
async fn r2_listing_against_worker_stub() {
    let stage = key("listing");
    let Some(store) = state_store(&stage) else { return };
    store.init().await.unwrap();

    for id in ["a", "b", "c"] {
        store.set(id, &state(id)).await.unwrap();
    }

    // The listing must page through the worker stub's object list and return
    // every id we wrote, sorted and decoded.
    let ids = collect(store.list().await.unwrap()).await;
    assert_eq!(ids, vec!["a", "b", "c"]);

    for id in ["a", "b", "c"] {
        store.delete(id).await.unwrap();
    }
}
