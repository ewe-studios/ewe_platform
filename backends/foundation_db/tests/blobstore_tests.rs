//! BlobStore integration tests for all backends.

use foundation_core::valtron::collect_one;
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
    let _: () = collect_one(storage.put_blob("test_key", test_data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));
}

#[test]
fn test_memory_blobstore_delete() {
    init_valtron();
    let storage = MemoryStorage::new();

    let test_data = b"To be deleted";
    let _: () = collect_one(storage.put_blob("to_delete", test_data).unwrap())
        .unwrap()
        .unwrap();

    let exists_before: bool = collect_one(storage.blob_exists("to_delete").unwrap())
        .unwrap()
        .unwrap();
    assert!(exists_before);

    let _: () = collect_one(storage.delete_blob("to_delete").unwrap())
        .unwrap()
        .unwrap();

    let exists_after: bool = collect_one(storage.blob_exists("to_delete").unwrap())
        .unwrap()
        .unwrap();
    assert!(!exists_after);

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob("to_delete").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, None);
}

#[test]
fn test_memory_blobstore_exists() {
    init_valtron();
    let storage = MemoryStorage::new();

    let exists: bool = collect_one(storage.blob_exists("nonexistent").unwrap())
        .unwrap()
        .unwrap();
    assert!(!exists);

    let test_data = b"Exists!";
    let _: () = collect_one(storage.put_blob("exists_test", test_data).unwrap())
        .unwrap()
        .unwrap();

    let exists: bool = collect_one(storage.blob_exists("exists_test").unwrap())
        .unwrap()
        .unwrap();
    assert!(exists);
}

#[test]
fn test_memory_blobstore_binary_data() {
    init_valtron();
    let storage = MemoryStorage::new();

    // Test with binary data including null bytes
    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    let _: () = collect_one(storage.put_blob("binary", &test_data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob("binary").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data));
}
