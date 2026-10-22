//! `BlobStore` integration tests for the R2 backend against a local
//! wrangler worker emulating the Cloudflare R2 REST API.
//!
//! Endpoint configuration lives in [`common`] — set `CF_INTEGRATION_TEST=1`
//! and optionally override `LOCAL_CF_API_BASE` to run these tests.

mod common;

use common::make_r2_store;
use foundation_core::valtron::valtron_test;
use foundation_db::{BlobStore, R2Store};

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

    storage.put_blob_async(&key, b"Hello, R2!").await.unwrap();
    assert_eq!(storage.get_blob_async(&key).await.unwrap(), Some(b"Hello, R2!".to_vec()));

    storage.delete_blob_async(&key).await.unwrap();
}

#[valtron_test]
async fn test_r2_blobstore_delete() {
    let Some(storage) = store() else { return };
    let key = key("test_delete");

    storage.put_blob_async(&key, b"To be deleted").await.unwrap();
    assert!(storage.blob_exists_async(&key).await.unwrap());

    storage.delete_blob_async(&key).await.unwrap();
    assert!(!storage.blob_exists_async(&key).await.unwrap());
}

#[valtron_test]
async fn test_r2_blobstore_exists() {
    let Some(storage) = store() else { return };
    let key = key("test_exists");

    assert!(!storage.blob_exists_async(&key).await.unwrap());
    storage.put_blob_async(&key, b"Exists!").await.unwrap();
    assert!(storage.blob_exists_async(&key).await.unwrap());

    storage.delete_blob_async(&key).await.unwrap();
}

#[valtron_test]
async fn test_r2_blobstore_binary_data() {
    let Some(storage) = store() else { return };
    let key = key("test_binary");

    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    storage.put_blob_async(&key, &test_data).await.unwrap();
    assert_eq!(storage.get_blob_async(&key).await.unwrap(), Some(test_data));

    storage.delete_blob_async(&key).await.unwrap();
}

#[valtron_test]
async fn test_r2_blobstore_delete_nonexistent() {
    let Some(storage) = store() else { return };
    // Deleting a missing key is not an error.
    storage.delete_blob_async(&key("test_del_none")).await.unwrap();
}
