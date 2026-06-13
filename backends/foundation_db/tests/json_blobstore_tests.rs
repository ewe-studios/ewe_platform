//! JSON file storage `BlobStore` tests.

use foundation_db::{BlobStore, JsonFileStorage};
use tempfile::TempDir;

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

#[test]
fn test_json_file_blobstore_put_get() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("blobs.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();

    let test_data = b"Hello from JSON file!";
    let _ = storage.put_blob("test_key", test_data)?
        .unwrap()
        .unwrap();

    // Reopen to verify persistence
    let storage2 = JsonFileStorage::new(&file_path).unwrap();
    let retrieved: Option<Vec<u8>> = storage2.get_blob("test_key")?
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));
}

#[test]
fn test_json_file_blobstore_delete() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("blobs.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();

    let test_data = b"To be deleted";
    let _ = storage.put_blob("to_delete", test_data)?
        .unwrap()
        .unwrap();

    let _ = storage.delete_blob("to_delete")?
        .unwrap()
        .unwrap();

    let _ = storage.get_blob("to_delete")?
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, None);
}

#[test]
fn test_json_file_blobstore_persistence() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("blobs.json");

    {
        let storage = JsonFileStorage::new(&file_path).unwrap();
        let test_data = b"Persist me!";
        let _ = storage.put_blob("persistent", test_data)?
            .unwrap()
            .unwrap();
    }

    // File should exist and contain data
    assert!(file_path.exists());

    let storage2 = JsonFileStorage::new(&file_path).unwrap();
    let retrieved: Option<Vec<u8>> = storage2.get_blob("persistent")?
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(b"Persist me!".to_vec()));
}
