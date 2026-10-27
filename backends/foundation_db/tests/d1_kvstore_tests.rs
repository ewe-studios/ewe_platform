//! `KeyValueStore` / `QueryStore` integration tests for the D1
//! backend against a local wrangler worker emulating the Cloudflare D1 REST
//! API. Skipped unless `CF_INTEGRATION_TEST=1` (see [`common`]).

mod common;

use common::make_d1_store;
use foundation_core::valtron::valtron_test;
use foundation_db::{KeyValueStore, QueryStore, D1Store, DataValue};
use futures_lite::StreamExt;

/// A KV-initialised store on the local worker, or `None` to skip.
async fn kv_store() -> Option<D1Store> {
    let Some(storage) = make_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return None;
    };
    storage.init_kv().await.unwrap();
    Some(storage)
}

fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn keys(storage: &D1Store, prefix: Option<&str>) -> Vec<String> {
    storage
        .list_keys(prefix)
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await
}

#[valtron_test]
async fn test_d1_kvstore_put_get() {
    let Some(storage) = kv_store().await else { return };
    let key = format!("test_put_get_{}", stamp());

    storage.set(&key, "Hello, D1!").await.unwrap();
    let retrieved: Option<String> = storage.get(&key).await.unwrap();
    assert_eq!(retrieved.as_deref(), Some("Hello, D1!"));

    storage.delete(&key).await.unwrap();
}

#[valtron_test]
async fn test_d1_kvstore_delete() {
    let Some(storage) = kv_store().await else { return };
    let key = format!("test_delete_{}", stamp());

    storage.set(&key, "To be deleted").await.unwrap();
    assert!(storage.exists(&key).await.unwrap());

    storage.delete(&key).await.unwrap();
    assert!(!storage.exists(&key).await.unwrap());
}

#[valtron_test]
async fn test_d1_kvstore_exists() {
    let Some(storage) = kv_store().await else { return };
    let key = format!("test_exists_{}", stamp());

    assert!(!storage.exists(&key).await.unwrap());
    storage.set(&key, "Exists!").await.unwrap();
    assert!(storage.exists(&key).await.unwrap());

    storage.delete(&key).await.unwrap();
}

#[valtron_test]
async fn test_d1_kvstore_list_keys() {
    let Some(storage) = kv_store().await else { return };

    storage.set("prefix:a", "1").await.unwrap();
    storage.set("prefix:b", "2").await.unwrap();
    storage.set("other:c", "3").await.unwrap();

    assert_eq!(keys(&storage, None).await.len(), 3);
    assert_eq!(keys(&storage, Some("prefix:")).await.len(), 2);

    for key in ["prefix:a", "prefix:b", "other:c"] {
        storage.delete(key).await.unwrap();
    }
}

#[valtron_test]
async fn test_d1_query_store() {
    let Some(storage) = make_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    let table_name = format!("test_table_{}", stamp());
    storage
        .execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (name TEXT PRIMARY KEY, value INTEGER)"
        ))
        .await
        .unwrap();

    storage
        .execute(
            &format!("INSERT INTO {table_name} (name, value) VALUES (?, ?)"),
            &[DataValue::Text("test".to_string()), DataValue::Integer(42)],
        )
        .await
        .unwrap();

    let rows: Vec<_> = storage
        .query(
            &format!("SELECT * FROM {table_name} WHERE name = ?"),
            &[DataValue::Text("test".to_string())],
        )
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await;
    assert_eq!(rows.len(), 1, "Should have found the inserted row");
    let name: String = rows[0].get_by_name("name").unwrap();
    let value: i64 = rows[0].get_by_name("value").unwrap();
    assert_eq!(name, "test");
    assert_eq!(value, 42);

    storage
        .execute_batch(&format!("DROP TABLE {table_name}"))
        .await
        .unwrap();
}
