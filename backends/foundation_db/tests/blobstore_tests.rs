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
    let _ = storage.put_blob("test_key", test_data)?
        .unwrap()
        .unwrap();

    let _ = storage.get_blob("test_key")?
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));
}

#[test]
fn test_memory_blobstore_delete() {
    init_valtron();
    let storage = MemoryStorage::new();

    let test_data = b"To be deleted";
    let _ = storage.put_blob("to_delete", test_data)?
        .unwrap()
        .unwrap();

    let _ = storage.blob_exists("to_delete")?
        .unwrap()
        .unwrap();
    assert!(exists_before);

    let _ = storage.delete_blob("to_delete")?
        .unwrap()
        .unwrap();

    let _ = storage.blob_exists("to_delete")?
        .unwrap()
        .unwrap();
    assert!(!exists_after);

    let _ = storage.get_blob("to_delete")?
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, None);
}

#[test]
fn test_memory_blobstore_exists() {
    init_valtron();
    let storage = MemoryStorage::new();

    let _ = storage.blob_exists("nonexistent")?
        .unwrap()
        .unwrap();
    assert!(!exists);

    let test_data = b"Exists!";
    let _ = storage.put_blob("exists_test", test_data)?
        .unwrap()
        .unwrap();

    let _ = storage.blob_exists("exists_test")?
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
    let _ = storage.put_blob("binary", &test_data)?
        .unwrap()
        .unwrap();

    let _ = storage.get_blob("binary")?
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data));
}
