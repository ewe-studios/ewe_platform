//! JsonFileStorage backend integration tests.

use foundation_db::{KeyValueStore, QueryStore, JsonFileStorage};
use tempfile::TempDir;

#[test]
fn test_json_file_storage_basic() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("store.json");

    let storage = JsonFileStorage::new(&path).unwrap();

    storage.set::<String>("test_key", "test_value".to_string()).unwrap();

    let value: Option<String> = storage.get("test_key").unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    let exists: bool = storage.exists("test_key").unwrap();
    assert!(exists);

    let not_exists: bool = storage.exists("nonexistent").unwrap();
    assert!(!not_exists);

    storage.delete("test_key").unwrap();

    let deleted: bool = storage.exists("test_key").unwrap();
    assert!(!deleted);
}

#[test]
fn test_json_file_storage_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("store.json");

    {
        let storage = JsonFileStorage::new(&path).unwrap();
        storage.set::<String>("persistent", "value".to_string()).unwrap();
    }

    {
        let storage2 = JsonFileStorage::new(&path).unwrap();
        let value: Option<String> = storage2.get("persistent").unwrap();
        assert_eq!(value, Some("value".to_string()));
    }
}

#[test]
fn test_json_file_storage_list_keys() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("store.json");

    let storage = JsonFileStorage::new(&path).unwrap();

    storage.set::<String>("key1", "v1".to_string()).unwrap();
    storage.set::<String>("key2", "v2".to_string()).unwrap();

    let keys: Vec<String> = storage.list_keys(None).unwrap()
        .flat_map(|s| match s { foundation_core::valtron::Stream::Next(Ok(r)) => vec![r], _ => vec![] })
        .collect();
    assert_eq!(keys.len(), 2);
}
