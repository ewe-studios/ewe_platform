//! `BlobStore` tests for the in-memory backend. Its futures are ready at
//! once, so they are driven with `futures_lite`'s `block_on`; no pool is needed.

use foundation_db::traits::BlobStore;
use foundation_db::MemoryStorage;
use futures_lite::future::block_on;

#[test]
fn test_memory_blobstore_put_get() {
    block_on(async {
        let storage = MemoryStorage::new();

        let test_data = b"Hello, BlobStore!";
        storage.put_blob("test_key", test_data).await.unwrap();

        let retrieved: Option<Vec<u8>> = storage.get_blob("test_key").await.unwrap();
        assert_eq!(retrieved, Some(test_data.to_vec()));
    });
}

#[test]
fn test_memory_blobstore_delete() {
    block_on(async {
        let storage = MemoryStorage::new();

        let test_data = b"To be deleted";
        storage.put_blob("del_key", test_data).await.unwrap();

        let exists_before: bool = storage.blob_exists("del_key").await.unwrap();
        assert!(exists_before);

        storage.delete_blob("del_key").await.unwrap();

        let exists_after: bool = storage.blob_exists("del_key").await.unwrap();
        assert!(!exists_after);

        let retrieved: Option<Vec<u8>> = storage.get_blob("del_key").await.unwrap();
        assert_eq!(retrieved, None);
    });
}

#[test]
fn test_memory_blobstore_exists() {
    block_on(async {
        let storage = MemoryStorage::new();

        let exists: bool = storage.blob_exists("nonexistent").await.unwrap();
        assert!(!exists);

        let test_data = b"Exists!";
        storage.put_blob("exists_key", test_data).await.unwrap();

        let exists: bool = storage.blob_exists("exists_key").await.unwrap();
        assert!(exists);
    });
}

#[test]
fn test_memory_blobstore_binary_data() {
    block_on(async {
        let storage = MemoryStorage::new();

        // Test with binary data including null bytes
        let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        storage.put_blob("binary_key", &test_data).await.unwrap();

        let retrieved: Option<Vec<u8>> = storage.get_blob("binary_key").await.unwrap();
        assert_eq!(retrieved, Some(test_data));
    });
}
