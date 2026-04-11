//! KeyValueStore integration tests for D1 backend using miniflare.
//!
//! These tests run against miniflare's local D1 emulation.
//! Requires miniflare to be installed and running.
//!
//! Set `D1_INTEGRATION_TEST=1` to enable these tests.
//! If miniflare is not available, tests will be skipped.

use foundation_core::valtron::collect_one;
use foundation_db::{DataValue, D1KeyValueStore, KeyValueStore, QueryStore};

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

/// Check if miniflare is available and D1 tests are enabled.
fn check_miniflare_available() -> bool {
    // Check if D1 integration tests are enabled
    if std::env::var("D1_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return false;
    }

    // Check if miniflare is running by checking the local D1 endpoint
    let local_d1_url = _env_var("LOCAL_D1_URL", "http://localhost:8789");

    // Try to ping the local D1 endpoint
    let response = std::process::Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &local_d1_url])
        .output();

    match response {
        Ok(output) => {
            let status = String::from_utf8_lossy(&output.stdout);
            status.trim() == "200" || status.trim() == "404"
        }
        Err(_) => false,
    }
}

fn _env_var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Create a D1 key-value store configured for local miniflare testing.
fn create_local_d1_store() -> Option<D1KeyValueStore> {
    if !check_miniflare_available() {
        return None;
    }

    let db_id = _env_var("LOCAL_D1_DATABASE_ID", "test-db");

    Some(D1KeyValueStore::new(
        "test-token", // Fake token for local testing
        "test-account", // Fake account for local testing
        &db_id,
        "test",
    ))
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
    let key = format!("test_put_get_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let _: () = collect_one(storage.set(&key, test_value).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<String> = collect_one(storage.get(&key).unwrap())
        .unwrap()
        .unwrap();
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
    let key = format!("test_delete_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let _: () = collect_one(storage.set(&key, test_value).unwrap())
        .unwrap()
        .unwrap();

    let exists_before: bool = collect_one(storage.exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(exists_before);

    let _: () = collect_one(storage.delete(&key).unwrap())
        .unwrap()
        .unwrap();

    let exists_after: bool = collect_one(storage.exists(&key).unwrap())
        .unwrap()
        .unwrap();
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

    let key = format!("test_exists_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let exists: bool = collect_one(storage.exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(!exists);

    let test_value = "Exists!";
    let _: () = collect_one(storage.set(&key, test_value).unwrap())
        .unwrap()
        .unwrap();

    let exists: bool = collect_one(storage.exists(&key).unwrap())
        .unwrap()
        .unwrap();
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

    let prefix = format!("test_list_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let key1 = format!("{prefix}_a");
    let key2 = format!("{prefix}_b");
    let key3 = format!("{prefix}_c");

    let _: () = collect_one(storage.set(&key1, "value_a").unwrap()).unwrap().unwrap();
    let _: () = collect_one(storage.set(&key2, "value_b").unwrap()).unwrap().unwrap();
    let _: () = collect_one(storage.set(&key3, "value_c").unwrap()).unwrap().unwrap();

    // List all keys with prefix
    let keys: String = collect_one(storage.list_keys(Some(&prefix)).unwrap())
        .unwrap()
        .unwrap();
    // Keys are returned as a comma-separated string, split them
    let keys: Vec<String> = keys.split(',').map(String::from).collect();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&key1));
    assert!(keys.contains(&key2));
    assert!(keys.contains(&key3));

    // Cleanup
    let _: () = collect_one(storage.delete(&key1).unwrap()).unwrap().unwrap();
    let _: () = collect_one(storage.delete(&key2).unwrap()).unwrap().unwrap();
    let _: () = collect_one(storage.delete(&key3).unwrap()).unwrap().unwrap();
}

#[test]
fn test_d1_kvstore_json_serialization() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
        age: u32,
        active: bool,
    }

    let key = format!("test_json_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let test_value = TestData {
        name: "Alice".to_string(),
        age: 30,
        active: true,
    };

    let _: () = collect_one(storage.set(&key, &test_value).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<TestData> = collect_one(storage.get(&key).unwrap())
        .unwrap()
        .unwrap();
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
    let table_name = format!("test_query_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let create_sql = format!(
        "CREATE TABLE {} (id INTEGER PRIMARY KEY, name TEXT NOT NULL, value INTEGER)",
        table_name
    );

    let _: () = collect_one(storage.execute_batch(&create_sql).unwrap())
        .unwrap()
        .unwrap();

    // Insert data
    let insert_sql = format!("INSERT INTO {} (name, value) VALUES (?, ?)", table_name);
    let _: u64 = collect_one(
        storage.execute(&insert_sql, &[DataValue::Text("test".to_string()), DataValue::Integer(42)])
            .unwrap()
    ).unwrap().unwrap();

    // Query data
    let select_sql = format!("SELECT * FROM {} WHERE name = ?", table_name);
    let row: foundation_db::SqlRow = collect_one(storage.query(&select_sql, &[DataValue::Text("test".to_string())]).unwrap())
        .unwrap()
        .unwrap();

    let name: String = row.get_by_name("name").unwrap();
    let value: i64 = row.get_by_name("value").unwrap();
    assert_eq!(name, "test");
    assert_eq!(value, 42);

    // Cleanup
    let drop_sql = format!("DROP TABLE {}", table_name);
    let _: () = collect_one(storage.execute_batch(&drop_sql).unwrap()).unwrap().unwrap();
}

#[test]
fn test_d1_kvstore_get_nonexistent() {
    init_valtron();
    let Some(storage) = create_local_d1_store() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    storage.init().unwrap();

    let key = format!("test_nonexistent_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let retrieved: Option<String> = collect_one(storage.get(&key).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, None);
}
