//! BlobStore integration tests for R2 backend using miniflare.
//!
//! These tests run against miniflare's local R2 emulation.
//! Requires miniflare to be installed and running.
//!
//! Set `R2_INTEGRATION_TEST=1` to enable these tests.
//! If miniflare is not available, tests will be skipped.

use foundation_core::valtron::collect_one;
use foundation_db::{BlobStore, R2BlobStore};

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

/// Check if miniflare is available and R2 tests are enabled.
fn check_miniflare_available() -> bool {
    // Check if R2 integration tests are enabled
    if std::env::var("R2_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return false;
    }

    // Check if miniflare is running by checking the local R2 endpoint
    let local_r2_url = _env_var("LOCAL_R2_URL", "http://localhost:8788");

    // Try to ping the local R2 endpoint
    let response = std::process::Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &local_r2_url])
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

/// Create an R2 blob store configured for local miniflare testing.
fn create_local_r2_store() -> Option<R2BlobStore> {
    if !check_miniflare_available() {
        return None;
    }

    let bucket = _env_var("LOCAL_R2_BUCKET", "test-bucket");

    Some(R2BlobStore::new(
        "test-token", // Fake token for local testing
        "test-account", // Fake account for local testing
        &bucket,
        "test",
    ))
}

#[test]
fn test_r2_blobstore_put_get() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available (set R2_INTEGRATION_TEST=1 and start miniflare)");
        return;
    };

    let test_data = b"Hello, R2!";
    let key = format!("test_put_get_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let _: () = collect_one(storage.put_blob(&key, test_data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob(&key).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data.to_vec()));

    // Cleanup
    let _: () = collect_one(storage.delete_blob(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_r2_blobstore_delete() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let test_data = b"To be deleted";
    let key = format!("test_delete_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let _: () = collect_one(storage.put_blob(&key, test_data).unwrap())
        .unwrap()
        .unwrap();

    let exists_before: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(exists_before);

    let _: () = collect_one(storage.delete_blob(&key).unwrap())
        .unwrap()
        .unwrap();

    let exists_after: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(!exists_after);
}

#[test]
fn test_r2_blobstore_exists() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_exists_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let exists: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(!exists);

    let test_data = b"Exists!";
    let _: () = collect_one(storage.put_blob(&key, test_data).unwrap())
        .unwrap()
        .unwrap();

    let exists: bool = collect_one(storage.blob_exists(&key).unwrap())
        .unwrap()
        .unwrap();
    assert!(exists);

    // Cleanup
    let _: () = collect_one(storage.delete_blob(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_r2_blobstore_binary_data() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_binary_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    // Test with binary data including null bytes
    let test_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
    let _: () = collect_one(storage.put_blob(&key, &test_data).unwrap())
        .unwrap()
        .unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(storage.get_blob(&key).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(retrieved, Some(test_data));

    // Cleanup
    let _: () = collect_one(storage.delete_blob(&key).unwrap()).unwrap().unwrap();
}

#[test]
fn test_r2_blobstore_delete_nonexistent() {
    init_valtron();
    let Some(storage) = create_local_r2_store() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    let key = format!("test_del_none_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    // Deleting non-existent key should succeed (no error)
    let result: Result<(), _> = collect_one(storage.delete_blob(&key).unwrap()).unwrap();
    assert!(result.is_ok());
}
