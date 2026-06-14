//! `KeyValueStore` integration tests for the D1 backend against a local
//! wrangler worker emulating the Cloudflare D1 REST API.

mod common;

use common::{init_valtron, make_d1_store};
use foundation_db::{DataValue, KeyValueStore, QueryStore};

fn create_local_d1_store() -> Option<foundation_db::D1Store> {
    make_d1_store()
}

#[test]
fn test_d1_kvstore_put_get() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };
    storage.init_kv().unwrap();

    let test_value = "Hello, D1!";
    let key = format!("test_put_get_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    storage.set(&key, test_value).unwrap();

    let retrieved: Option<String> = storage.get(&key).unwrap();
    assert_eq!(retrieved, Some(test_value.to_string()));

    storage.delete(&key).unwrap();
}

#[test]
fn test_d1_kvstore_delete() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };
    storage.init_kv().unwrap();

    let test_value = "To be deleted";
    let key = format!("test_delete_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    storage.set(&key, test_value).unwrap();

    let exists_before: bool = storage.exists(&key).unwrap();
    assert!(exists_before);

    storage.delete(&key).unwrap();

    let exists_after: bool = storage.exists(&key).unwrap();
    assert!(!exists_after);
}

#[test]
fn test_d1_kvstore_exists() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };
    storage.init_kv().unwrap();

    let key = format!("test_exists_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let exists: bool = storage.exists(&key).unwrap();
    assert!(!exists);

    storage.set(&key, "Exists!").unwrap();

    let exists: bool = storage.exists(&key).unwrap();
    assert!(exists);

    storage.delete(&key).unwrap();
}

#[test]
fn test_d1_kvstore_list_keys() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };
    storage.init_kv().unwrap();

    storage.set("prefix:a", "1").unwrap();
    storage.set("prefix:b", "2").unwrap();
    storage.set("other:c", "3").unwrap();

    let keys: Vec<String> = storage.list_keys(None).unwrap()
        .flat_map(|s| match s { foundation_core::valtron::Stream::Next(Ok(r)) => vec![r], _ => vec![] })
        .collect();
    assert_eq!(keys.len(), 3);

    let keys: Vec<String> = storage.list_keys(Some("prefix:")).unwrap()
        .flat_map(|s| match s { foundation_core::valtron::Stream::Next(Ok(r)) => vec![r], _ => vec![] })
        .collect();
    assert_eq!(keys.len(), 2);

    storage.delete("prefix:a").unwrap();
    storage.delete("prefix:b").unwrap();
    storage.delete("other:c").unwrap();
}

#[test]
fn test_d1_query_store() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    let table_name = format!("test_table_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let create_sql = format!("CREATE TABLE IF NOT EXISTS {} (name TEXT PRIMARY KEY, value INTEGER)", table_name);

    storage.execute_batch(&create_sql).unwrap();

    let insert_sql = format!("INSERT INTO {} (name, value) VALUES (?, ?)", table_name);
    storage.execute(&insert_sql, &[DataValue::Text("test".to_string()), DataValue::Integer(42)]).unwrap();

    let select_sql = format!("SELECT * FROM {} WHERE name = ?", table_name);
    let mut rows = storage.query(&select_sql, &[DataValue::Text("test".to_string())]).unwrap();
    
    let mut found = false;
    for item in &mut rows {
        if let foundation_core::valtron::Stream::Next(Ok(row)) = item {
            let name: String = row.get_by_name("name").unwrap();
            let value: i64 = row.get_by_name("value").unwrap();
            assert_eq!(name, "test");
            assert_eq!(value, 42);
            found = true;
        }
    }
    assert!(found, "Should have found the inserted row");

    let drop_sql = format!("DROP TABLE {}", table_name);
    storage.execute_batch(&drop_sql).unwrap();
}


