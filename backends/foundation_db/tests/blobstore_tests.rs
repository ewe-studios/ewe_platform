//! `BlobStore` integration tests for all backends.

use foundation_db::{BlobStore, MemoryStorage};

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

#[test]
fn test_memory_blobstore_put_get() {
    init_valtron();
    let storage = MemoryStorage::new();

    let test_data = b"Hello, BlobStore!";
    storage.put_blob("test_key", test_data).unwrap();

    let retrieved: Option<Vec<u8>> = storage.get_blob("test_key").unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));
}

#[test]
fn test_memory_blobstore_delete() {
    init_valtron();
    let storage = MemoryStorage::new();

    let test_data = b"To be deleted";
    storage.put_blob("del_key", test_data).unwrap();

    let exists_before: bool = storage.blob_exists("del_key").unwrap();
    assert!(exists_before);

    storage.delete_blob("del_key").unwrap();

    let exists_after: bool = storage.blob_exists("del_key").unwrap();
    assert!(!exists_after);

    let retrieved: Option<Vec<u8>> = storage.get_blob("del_key").unwrap();
    assert_eq!(retrieved, None);
}

#[test]
fn test_memory_blobstore_exists() {
    init_valtron();
    let storage = MemoryStorage::new();

    let exists: bool = storage.blob_exists("nonexistent").unwrap();
    assert!(!exists);

    let test_data = b"Exists!";
    storage.put_blob("exists_key", test_data).unwrap();

    let exists: bool = storage.blob_exists("exists_key").unwrap();
    assert!(exists);
}

#[test]
fn test_memory_blobstore_binary_data() {
    init_valtron();
    let storage = MemoryStorage::new();

    // Test with binary data including null bytes
    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    storage.put_blob("binary_key", &test_data).unwrap();

    let retrieved: Option<Vec<u8>> = storage.get_blob("binary_key").unwrap();
    assert_eq!(retrieved, Some(test_data));
}
