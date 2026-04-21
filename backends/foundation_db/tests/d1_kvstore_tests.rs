//! `KeyValueStore` integration tests for the D1 backend against a local
//! wrangler worker emulating the Cloudflare D1 REST API.
//!
//! Endpoint configuration lives in [`common`] — set `CF_INTEGRATION_TEST=1`
//! and optionally override `LOCAL_CF_API_BASE` to run these tests.

mod common;

use common::{init_valtron, make_d1_store};
use foundation_core::valtron::collect_one;
use foundation_core::valtron::collect_result;
use foundation_db::{DataValue, KeyValueStore, QueryStore};

fn create_local_d1_store() -> Option<foundation_db::D1KeyValueStore> {
    make_d1_store()
}

#[test]
fn test_d1_kvstore_put_get() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available (set D1_INTEGRATION_TEST=1 and start miniflare)");
        return;
    };

    // Initialize schema
    storage.init().unwrap();

    let test_value = "Hello, D1!";
    let key = format!(
        "test_put_get_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let _: () = collect_one(storage.set(&key, test_value).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<String> = collect_one(storage.get(&key).unwrap()).unwrap().unwrap();
    assert_eq!(retrieved, Some(test_value.to_string()));

    // Cleanup
    let _: () = collect_one(storage.delete(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_d1_kvstore_delete() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    let test_value = "To be deleted";
    let key = format!(
        "test_delete_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let _: () = collect_one(storage.set(&key, test_value).unwrap())
        .unwrap()
        .unwrap();

    let exists_before: bool = collect_one(storage.exists(&key).unwrap()).unwrap().unwrap();
    assert!(exists_before);

    let _: () = collect_one(storage.delete(&key).unwrap()).unwrap().unwrap();

    let exists_after: bool = collect_one(storage.exists(&key).unwrap()).unwrap().unwrap();
    assert!(!exists_after);
}

#[test]
fn test_d1_kvstore_exists() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    let key = format!(
        "test_exists_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let exists: bool = collect_one(storage.exists(&key).unwrap()).unwrap().unwrap();
    assert!(!exists);

    let test_value = "Exists!";
    let _: () = collect_one(storage.set(&key, test_value).unwrap())
        .unwrap()
        .unwrap();

    let exists: bool = collect_one(storage.exists(&key).unwrap()).unwrap().unwrap();
    assert!(exists);

    // Cleanup
    let _: () = collect_one(storage.delete(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_d1_kvstore_list_keys() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    let prefix = format!(
        "test_list_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let key1 = format!("{prefix}_a");
    let key2 = format!("{prefix}_b");
    let key3 = format!("{prefix}_c");

    let _: () = collect_one(storage.set(&key1, "value_a").unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.set(&key2, "value_b").unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.set(&key3, "value_c").unwrap())
        .unwrap()
        .unwrap();

    // List all keys with prefix
    let listed: Vec<String> = collect_result(storage.list_keys(Some(&prefix)).unwrap())
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(listed.len(), 3);
    assert!(listed.contains(&key1));
    assert!(listed.contains(&key2));
    assert!(listed.contains(&key3));

    // Cleanup
    let _: () = collect_one(storage.delete(&key1).unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.delete(&key2).unwrap())
        .unwrap()
        .unwrap();
    let _: () = collect_one(storage.delete(&key3).unwrap())
        .unwrap()
        .unwrap();
}

#[test]
fn test_d1_kvstore_json_serialization() {
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
        age: u32,
        active: bool,
    }

    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    let key = format!(
        "test_json_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let test_value = TestData {
        name: "Alice".to_string(),
        age: 30,
        active: true,
    };

    let _: () = collect_one(storage.set(&key, &test_value).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<TestData> = collect_one(storage.get(&key).unwrap()).unwrap().unwrap();
    assert_eq!(retrieved, Some(test_value));

    // Cleanup
    let _: () = collect_one(storage.delete(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_d1_query_store() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    // Create a test table
    let table_name = format!(
        "test_query_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let create_sql = format!(
        "CREATE TABLE {table_name} (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value INTEGER)"
    );

    let _: () = collect_one(storage.execute_batch(&create_sql).unwrap())
        .unwrap()
        .unwrap();

    // Insert data
    let insert_sql = format!("INSERT INTO {table_name} (name, value) VALUES (?, ?)");
    let _: u64 = collect_one(
        storage
            .execute(
                &insert_sql,
                &[DataValue::Text("test".to_string()), DataValue::Integer(42)],
            )
            .unwrap(),
    )
    .unwrap()
    .unwrap();

    // Query data
    let select_sql = format!("SELECT * FROM {table_name} WHERE name = ?");
    let row: foundation_db::SqlRow = collect_one(
        storage
            .query(&select_sql, &[DataValue::Text("test".to_string())])
            .unwrap(),
    )
    .unwrap()
    .unwrap();

    let name: String = row.get_by_name("name").unwrap();
    let value: i64 = row.get_by_name("value").unwrap();
    assert_eq!(name, "test");
    assert_eq!(value, 42);

    // Cleanup
    let drop_sql = format!("DROP TABLE {table_name}");
    let _: () = collect_one(storage.execute_batch(&drop_sql).unwrap())
        .unwrap()
        .unwrap();
}

#[test]
fn test_d1_kvstore_get_nonexistent() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    let key = format!(
        "test_nonexistent_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let retrieved: Option<String> = collect_one(storage.get(&key).unwrap()).unwrap().unwrap();
    assert_eq!(retrieved, None);
}
