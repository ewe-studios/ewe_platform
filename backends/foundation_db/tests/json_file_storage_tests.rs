//! `JsonFileStorage` backend integration tests, through its async storage
//! traits (it implements only the `Async*` ones; see `blocking_tests.rs` for
//! the sync view).

use foundation_db::{
    KeyValueStore, QueryStore, RateLimiterStore, JsonFileStorage, StorageError,
};
use futures_lite::future::block_on;
use tempfile::TempDir;

#[test]
fn test_json_file_storage_basic() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("store.json");

    let storage = JsonFileStorage::new(&path).unwrap();

    block_on(async {
        storage
            .set_async::<String>("test_key", "test_value".to_string())
            .await
            .unwrap();

        let value: Option<String> = storage.get_async("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        assert!(storage.exists_async("test_key").await.unwrap());
        assert!(!storage.exists_async("nonexistent").await.unwrap());

        storage.delete_async("test_key").await.unwrap();
        assert!(!storage.exists_async("test_key").await.unwrap());
    });
}

#[test]
fn test_json_file_storage_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("store.json");

    {
        let storage = JsonFileStorage::new(&path).unwrap();
        block_on(storage.set_async::<String>("persistent", "value".to_string())).unwrap();
    }

    {
        let storage2 = JsonFileStorage::new(&path).unwrap();
        let value: Option<String> = block_on(storage2.get_async("persistent")).unwrap();
        assert_eq!(value, Some("value".to_string()));
    }
}

#[test]
fn test_json_file_storage_list_keys() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("store.json");

    let storage = JsonFileStorage::new(&path).unwrap();

    let keys = block_on(async {
        storage.set_async::<String>("key1", "v1".to_string()).await.unwrap();
        storage.set_async::<String>("key2", "v2".to_string()).await.unwrap();
        storage
            .list_keys_async(None)
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap()
    });
    assert_eq!(keys.len(), 2);
}

/// `JsonFileStorage` has no SQL engine: its async query methods report that
/// instead of pretending to run the statement.
#[test]
fn test_json_file_storage_query_is_unsupported() {
    let temp_dir = TempDir::new().unwrap();
    let storage = JsonFileStorage::new(temp_dir.path().join("store.json")).unwrap();

    block_on(async {
        assert!(matches!(
            storage.query_async("SELECT 1", &[]).await,
            Err(StorageError::Generic(_))
        ));
        assert!(matches!(
            storage.execute_async("DELETE FROM t", &[]).await,
            Err(StorageError::Generic(_))
        ));
        assert!(matches!(
            storage.execute_batch_async("CREATE TABLE t (x)").await,
            Err(StorageError::Generic(_))
        ));
    });
}

/// Rate limiting is not supported by `JsonFileStorage`: every async method
/// fails rather than silently allowing.
#[test]
fn test_json_file_storage_rate_limit_is_unsupported() {
    let temp_dir = TempDir::new().unwrap();
    let storage = JsonFileStorage::new(temp_dir.path().join("store.json")).unwrap();

    block_on(async {
        assert!(matches!(
            storage.check_rate_limit_async("k", 5, 60).await,
            Err(StorageError::Generic(_))
        ));
        assert!(matches!(
            storage.record_rate_limit_async("k", 60).await,
            Err(StorageError::Generic(_))
        ));
        assert!(matches!(
            storage.reset_rate_limit_async("k").await,
            Err(StorageError::Generic(_))
        ));
    });
}
