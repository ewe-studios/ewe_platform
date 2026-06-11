#![cfg(feature = "vfs-d1")]

use foundation_nativeapis::native::vfs::d1_delta::D1Delta;
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, VfsFile, VfsFileSystem,
};

/// Check if local CF emulator is available (same pattern as foundation_db tests)
fn is_local_cf_available() -> bool {
    if std::env::var("CF_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return false;
    }
    let base = std::env::var("LOCAL_CF_API_BASE").unwrap_or_else(|_| "http://localhost:8789".to_string());
    let response = std::process::Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &base])
        .output();
    match response {
        Ok(output) => {
            let status = String::from_utf8_lossy(&output.stdout);
            let status = status.trim();
            status == "200" || status == "404"
        }
        Err(_) => false,
    }
}

/// Initialize valtron pool for tests
static POOL_GUARD: std::sync::Mutex<Option<foundation_core::valtron::PoolGuard>> =
    std::sync::Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

/// Create D1Delta pointed at local emulator, or None if unavailable
fn make_d1_delta() -> Option<D1Delta> {
    if !is_local_cf_available() {
        return None;
    }
    let base = std::env::var("LOCAL_CF_API_BASE").unwrap_or_else(|_| "http://localhost:8789".to_string());
    let db_id = std::env::var("LOCAL_D1_DATABASE_ID").unwrap_or_else(|_| "test-db".to_string());
    D1Delta::new_with_base_url("test-token", "test-account", &db_id, Some(&base)).ok()
}

// ── API Surface Tests (always run) ──

#[test]
fn test_d1_delta_new_with_credentials() {
    let result = D1Delta::new("test-token", "test-account", "test-db");
    assert!(result.is_ok());
}

#[test]
fn test_d1_delta_from_env_no_creds() {
    let result = D1Delta::from_env();
    assert!(result.is_err());
}

#[test]
fn test_d1_delta_is_vfs_file_system() {
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    assert_vfs_fs::<D1Delta>();
}

#[test]
fn test_d1_delta_is_delta_store() {
    fn assert_delta<T: DeltaStore>() {}
    assert_delta::<D1Delta>();
}

// ── Integration Tests (require local CF emulator) ──

#[test]
fn test_d1_kvstore_put_get() {
    init_valtron();
    let Some(delta) = make_d1_delta() else {
        println!("Skipping D1 test - miniflare not available (set CF_INTEGRATION_TEST=1)");
        return;
    };

    delta.create("/test_kv.txt", 0o644).unwrap();
    delta.write_file("/test_kv.txt", b"Hello, D1!").unwrap();
    let data = delta.read_file("/test_kv.txt").unwrap();
    assert_eq!(&data, b"Hello, D1!");
}

#[test]
fn test_d1_delta_exists() {
    init_valtron();
    let Some(delta) = make_d1_delta() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    delta.create("/exists_test.txt", 0o644).unwrap();
    assert!(delta.exists("/exists_test.txt").unwrap());
    assert!(!delta.exists("/nonexistent.txt").unwrap());
}

#[test]
fn test_d1_delta_whiteout() {
    init_valtron();
    let Some(delta) = make_d1_delta() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    delta.add_whiteout("/whiteout_test.txt", 1).unwrap();
    assert!(delta.is_whiteout("/whiteout_test.txt").unwrap().is_some());
    delta.remove_whiteout("/whiteout_test.txt").unwrap();
    assert!(delta.is_whiteout("/whiteout_test.txt").unwrap().is_none());
}

#[test]
fn test_d1_delta_read_write() {
    init_valtron();
    let Some(delta) = make_d1_delta() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    // Create and write
    let file = delta.create("/rw_test.txt", 0o644).unwrap();
    file.write_at(b"test content", 0).unwrap();

    // Read back
    let file = delta.open("/rw_test.txt", OpenMode::Read).unwrap();
    let mut buf = [0u8; 12];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 12);
    assert_eq!(&buf, b"test content");
}
