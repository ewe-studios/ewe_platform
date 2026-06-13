//! Memory storage backend integration tests.

use foundation_db::{KeyValueStore, MemoryStorage};
use serde::{Deserialize, Serialize};

#[test]
fn test_memory_storage_basic() {
    let storage = MemoryStorage::new();

    storage.set("test_key", "test_value").unwrap();

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
fn test_memory_storage_list_keys() {
    let storage = MemoryStorage::new();

    storage.set("prefix:key1", "value1").unwrap();
    storage.set("prefix:key2", "value2").unwrap();
    storage.set("other:key3", "value3").unwrap();

    let keys: Vec<String> = storage
        .list_keys(None)
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.len(), 3);

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
fn test_memory_storage_complex_value() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct TestData {
        name: String,
        count: u32,
    }

    let storage = MemoryStorage::new();

    let data = TestData {
        name: "test".to_string(),
        count: 42,
    };

    storage.set("complex", data.clone()).unwrap();

    let retrieved: Option<TestData> = storage.get("complex").unwrap();
    assert_eq!(retrieved, Some(data));
}
