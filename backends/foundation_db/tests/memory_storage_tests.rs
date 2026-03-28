//! Memory storage backend integration tests.

use foundation_core::valtron::collect_one;
use foundation_db::backends::MemoryStorage;
use foundation_db::storage_provider::KeyValueStore;

#[test]
fn test_memory_storage_basic() {
    let storage = MemoryStorage::new();

    // Test set and get
    collect_one(storage.set("test_key", "test_value").unwrap())
        .unwrap()
        .unwrap();

    let value: Option<String> = collect_one(storage.get("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    // Test exists
    assert!(collect_one(storage.exists("test_key").unwrap())
        .unwrap()
        .unwrap());
    assert!(!collect_one(storage.exists("nonexistent").unwrap())
        .unwrap()
        .unwrap());

    // Test delete
    collect_one(storage.delete("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert!(!collect_one(storage.exists("test_key").unwrap())
        .unwrap()
        .unwrap());
}

#[test]
fn test_memory_storage_list_keys() {
    let storage = MemoryStorage::new();

    collect_one(storage.set("prefix:key1", "value1").unwrap())
        .unwrap()
        .unwrap();
    collect_one(storage.set("prefix:key2", "value2").unwrap())
        .unwrap()
        .unwrap();
    collect_one(storage.set("other:key3", "value3").unwrap())
        .unwrap()
        .unwrap();

    // List all keys - flat_map to extract Result from Stream, then collect
    let keys: Result<Vec<String>, _> = storage
        .list_keys(None)
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(result) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.unwrap().len(), 3);

    // List keys with prefix
    let keys: Result<Vec<String>, _> = storage
        .list_keys(Some("prefix:"))
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(result) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.unwrap().len(), 2);
    assert!(keys.as_ref().unwrap().contains(&"prefix:key1".to_string()));
    assert!(keys.as_ref().unwrap().contains(&"prefix:key2".to_string()));
}

#[test]
fn test_memory_storage_complex_value() {
    let storage = MemoryStorage::new();

    #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
        count: u32,
    }

    let data = TestData {
        name: "test".to_string(),
        count: 42,
    };

    collect_one(storage.set("complex", &data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<TestData> = collect_one(storage.get("complex").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(data));
}
