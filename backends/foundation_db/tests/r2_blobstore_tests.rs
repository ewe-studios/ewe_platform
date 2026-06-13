//! `BlobStore` integration tests for the R2 backend against a local wrangler
//! worker emulating the Cloudflare R2 REST API.
//!
//! Endpoint configuration lives in [`common`] — set `CF_INTEGRATION_TEST=1`
//! and optionally override `LOCAL_CF_API_BASE` to run these tests.

mod common;

use common::{init_valtron, make_r2_store};
use foundation_db::BlobStore;

fn create_local_r2_store() -> Option<foundation_db::R2Store> {
    make_r2_store()
}

#[test]
fn test_r2_blobstore_put_get() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available (set R2_INTEGRATION_TEST=1 and start miniflare)");
        return;
    };

    let test_data = b"Hello, R2!";
    let key = format!("test_put_get_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    storage.put_blob(&key, test_data).unwrap();

    let retrieved: Option<Vec<u8>> = storage.get_blob(&key).unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));

    storage.delete_blob(&key).unwrap();
}

#[test]
fn test_r2_blobstore_delete() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let test_data = b"To be deleted";
    let key = format!("test_delete_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    storage.put_blob(&key, test_data).unwrap();

    let exists_before: bool = storage.blob_exists(&key).unwrap();
    assert!(exists_before);

    storage.delete_blob(&key).unwrap();

    let exists_after: bool = storage.blob_exists(&key).unwrap();
    assert!(!exists_after);
}

#[test]
fn test_r2_blobstore_exists() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_exists_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let exists: bool = storage.blob_exists(&key).unwrap();
    assert!(!exists);

    let test_data = b"Exists!";
    storage.put_blob(&key, test_data).unwrap();

    let exists: bool = storage.blob_exists(&key).unwrap();
    assert!(exists);

    storage.delete_blob(&key).unwrap();
}

#[test]
fn test_r2_blobstore_binary_data() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_binary_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    storage.put_blob(&key, &test_data).unwrap();

    let retrieved: Option<Vec<u8>> = storage.get_blob(&key).unwrap();
    assert_eq!(retrieved, Some(test_data));

    storage.delete_blob(&key).unwrap();
}

#[test]
fn test_r2_blobstore_delete_nonexistent() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_del_none_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    // Deleting non-existent key should succeed (no error)
    let result = storage.delete_blob(&key);
    assert!(result.is_ok());
}
