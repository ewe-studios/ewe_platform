//! JSON file storage `AsyncBlobStore` tests.

use foundation_core::valtron::valtron_test;
use foundation_db::{AsyncBlobStore, JsonFileStorage};
use tempfile::TempDir;

#[valtron_test]
async fn test_json_file_blobstore_put_get() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("blobs.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();
    let test_data = b"Hello from JSON file!";
    storage.put_blob_async("test_key", test_data).await.unwrap();

    // Reopen to verify persistence
    let storage2 = JsonFileStorage::new(&file_path).unwrap();
    let retrieved = storage2.get_blob_async("test_key").await.unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));
}

#[valtron_test]
async fn test_json_file_blobstore_delete() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("blobs.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();
    storage.put_blob_async("del_key", b"To be deleted").await.unwrap();
    assert!(storage.blob_exists_async("del_key").await.unwrap());

    storage.delete_blob_async("del_key").await.unwrap();
    assert_eq!(storage.get_blob_async("del_key").await.unwrap(), None);
}

#[valtron_test]
async fn test_json_file_blobstore_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("blobs.json");

    {
        let storage = JsonFileStorage::new(&file_path).unwrap();
        storage.put_blob_async("persist_key", b"Persist me!").await.unwrap();
    }

    // File should exist and contain data
    assert!(file_path.exists());

    let storage2 = JsonFileStorage::new(&file_path).unwrap();
    let retrieved = storage2.get_blob_async("persist_key").await.unwrap();
    assert_eq!(retrieved, Some(b"Persist me!".to_vec()));
}
