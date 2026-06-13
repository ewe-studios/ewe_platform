//! `BlobStore` integration tests for the R2 backend against a local wrangler
//! worker emulating the Cloudflare R2 REST API.
//!
//! Endpoint configuration lives in [`common`] — set `CF_INTEGRATION_TEST=1`
//! and optionally override `LOCAL_CF_API_BASE` to run these tests.

mod common;

use common::{init_valtron, make_r2_store};
use foundation_core::valtron::collect_one;
use foundation_db::BlobStore;

fn create_local_r2_store() -> Option<foundation_db::R2BlobStore> {
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

    let _: () = collect_one(storage.put_blob(&key, test_data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob(&key).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));

    // Cleanup
    let _: () = collect_one(storage.delete_blob(&key).unwrap()).unwrap().unwrap();
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

    let _: () = collect_one(storage.put_blob(&key, test_data).unwrap())
        .unwrap()
        .unwrap();

    let exists_before: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(exists_before);

    let _: () = collect_one(storage.delete_blob(&key).unwrap())
        .unwrap()
        .unwrap();

    let exists_after: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
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

    let exists: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(!exists);

    let test_data = b"Exists!";
    let _: () = collect_one(storage.put_blob(&key, test_data).unwrap())
        .unwrap()
        .unwrap();

    let exists: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(exists);

    // Cleanup
    let _: () = collect_one(storage.delete_blob(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_r2_blobstore_binary_data() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_binary_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    // Test with binary data including null bytes
    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    let _: () = collect_one(storage.put_blob(&key, &test_data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob(&key).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data));

    // Cleanup
    let _: () = collect_one(storage.delete_blob(&key).unwrap()).unwrap().unwrap();
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
    let result: Result<(), _> = collect_one(storage.delete_blob(&key).unwrap()).unwrap();
    assert!(result.is_ok());
}
