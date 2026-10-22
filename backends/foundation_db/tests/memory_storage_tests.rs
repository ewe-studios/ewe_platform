//! Memory storage backend integration tests.
//!
//! `MemoryStorage` implements only the async store traits; its futures are
//! ready at once, so these tests drive them with `futures_lite`'s `block_on`.

use foundation_db::traits::KeyValueStore;
use foundation_db::MemoryStorage;
use futures_lite::future::block_on;
use serde::{Deserialize, Serialize};

#[test]
fn test_memory_storage_basic() {
    block_on(async {
        let storage = MemoryStorage::new();

        storage.set_async("test_key", "test_value").await.unwrap();

        let value: Option<String> = storage.get_async("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        assert!(storage.exists_async("test_key").await.unwrap());
        assert!(!storage.exists_async("nonexistent").await.unwrap());

        storage.delete_async("test_key").await.unwrap();

        assert!(!storage.exists_async("test_key").await.unwrap());
    });
}

#[test]
fn test_memory_storage_list_keys() {
    block_on(async {
        let storage = MemoryStorage::new();

        storage.set_async("prefix:key1", "value1").await.unwrap();
        storage.set_async("prefix:key2", "value2").await.unwrap();
        storage.set_async("other:key3", "value3").await.unwrap();

        let keys = storage
            .list_keys_async(None)
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap();
        assert_eq!(keys.len(), 3);

        let keys = storage
            .list_keys_async(Some("prefix:"))
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix:key1".to_string()));
        assert!(keys.contains(&"prefix:key2".to_string()));
    });
}

#[test]
fn test_memory_storage_complex_value() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct TestData {
        name: String,
        count: u32,
    }

    block_on(async {
        let storage = MemoryStorage::new();

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        storage.set_async("complex", data.clone()).await.unwrap();

        let retrieved: Option<TestData> = storage.get_async("complex").await.unwrap();
        assert_eq!(retrieved, Some(data));
    });
}
