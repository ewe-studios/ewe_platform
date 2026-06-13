//! JSON file storage backend integration tests.

use foundation_core::valtron::collect_one;
use foundation_db::{JsonFileStorage, KeyValueStore};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[test]
fn test_json_file_storage_basic() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();

    // Test set and get
    let _: () = collect_one(storage.set("test_key", "test_value").unwrap())
        .unwrap()
        .unwrap();

    let value: Option<String> = collect_one(storage.get("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    // Test exists
    let exists: bool = collect_one(storage.exists("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert!(exists);

    let not_exists: bool = collect_one(storage.exists("nonexistent").unwrap())
        .unwrap()
        .unwrap();
    assert!(!not_exists);

    // Test delete
    let _: () = collect_one(storage.delete("test_key").unwrap())
        .unwrap()
        .unwrap();

    let deleted: bool = collect_one(storage.exists("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert!(!deleted);
}

#[test]
fn test_json_file_storage_list_keys() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();

    let _: () = collect_one(storage.set("prefix:key1", "value1").unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.set("prefix:key2", "value2").unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.set("other:key3", "value3").unwrap())
        .unwrap()
        .unwrap();

    // List all keys - flat_map to extract Result from Stream, then collect
    let keys: Vec<String> = storage
        .list_keys(None)
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.len(), 3);

    // List keys with prefix
    let keys: Vec<String> = storage
        .list_keys(Some("prefix:"))
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"prefix:key1".to_string()));
    assert!(keys.contains(&"prefix:key2".to_string()));
}

#[test]
fn test_json_file_storage_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.json");

    // Create storage and add data
    let storage = JsonFileStorage::new(&file_path).unwrap();
    let _: () = collect_one(storage.set("key1", "value1").unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.set("key2", "value2").unwrap())
        .unwrap()
        .unwrap();

    // Verify file was created
    assert!(file_path.exists());

    // Create new storage instance from same file
    let storage2 = JsonFileStorage::new(&file_path).unwrap();

    // Data should persist
    let value1: Option<String> = collect_one(storage2.get("key1").unwrap()).unwrap().unwrap();
    assert_eq!(value1, Some("value1".to_string()));

    let value2: Option<String> = collect_one(storage2.get("key2").unwrap()).unwrap().unwrap();
    assert_eq!(value2, Some("value2".to_string()));
}

#[test]
fn test_json_file_storage_complex_value() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
        count: u32,
    }

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();

    let data = TestData {
        name: "test".to_string(),
        count: 42,
    };

    let _: () = collect_one(storage.set("complex", &data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<TestData> = collect_one(storage.get("complex").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(data));
}

#[test]
fn test_json_file_storage_atomic_writes() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.json");

    let storage = JsonFileStorage::new(&file_path).unwrap();

    // Add initial data
    let _: () = collect_one(storage.set("key1", "value1").unwrap())
        .unwrap()
        .unwrap();

    // Verify no temp file left behind
    let temp_path = file_path.with_extension("json.tmp");
    assert!(!temp_path.exists());

    // Verify main file exists
    assert!(file_path.exists());
}
