#![cfg(feature = "vfs-d1")]

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::native::vfs::d1_delta::D1Delta;
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
use foundation_nativeapis::shared::vfs::{DeltaStore, VfsFileSystem};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

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

/// Create a SyncFs<D1Delta> pointed at local emulator, or None if unavailable
fn make_sync_d1() -> Option<SyncFs<D1Delta>> {
    if !is_local_cf_available() {
        return None;
    }
    let base = std::env::var("LOCAL_CF_API_BASE").unwrap_or_else(|_| "http://localhost:8789".to_string());
    let db_id = std::env::var("LOCAL_D1_DATABASE_ID").unwrap_or_else(|_| "test-db".to_string());
    let delta = D1Delta::new_with_base_url("test-token", "test-account", &db_id, Some(&base)).ok()?;
    Some(SyncFs::new(delta))
}

// ── API Surface Tests (always run, no valtron needed) ──

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

// ── Integration Tests through valtron sync bridge ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_d1_kvstore_put_get() {
    let _guard = init_pool();
    let Some(sync) = make_sync_d1() else {
        println!("Skipping D1 test - miniflare not available (set CF_INTEGRATION_TEST=1)");
        return;
    };

    sync.create("/test_kv.txt", 0o644).unwrap();
    sync.write_file("/test_kv.txt", b"Hello, D1!").unwrap();
    let data = sync.read_file("/test_kv.txt").unwrap();
    assert_eq!(&data, b"Hello, D1!");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_d1_exists() {
    let _guard = init_pool();
    let Some(sync) = make_sync_d1() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    sync.create("/exists_test.txt", 0o644).unwrap();
    assert!(sync.exists("/exists_test.txt").unwrap());
    assert!(!sync.exists("/nonexistent.txt").unwrap());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_d1_whiteout() {
    let _guard = init_pool();
    let Some(sync) = make_sync_d1() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    sync.add_whiteout("/whiteout_test.txt", 1).unwrap();
    assert!(sync.is_whiteout("/whiteout_test.txt").unwrap().is_some());
    sync.remove_whiteout("/whiteout_test.txt").unwrap();
    assert!(sync.is_whiteout("/whiteout_test.txt").unwrap().is_none());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_d1_read_write() {
    let _guard = init_pool();
    let Some(sync) = make_sync_d1() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    sync.create("/rw_test.txt", 0o644).unwrap();
    sync.write_file("/rw_test.txt", b"test content").unwrap();
    let data = sync.read_file("/rw_test.txt").unwrap();
    assert_eq!(&data, b"test content");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_d1_flush_and_reset() {
    let _guard = init_pool();
    let Some(sync) = make_sync_d1() else {
        println!("Skipping D1 test - miniflare not available");
        return;
    };

    sync.create("/reset_test.txt", 0o644).unwrap();
    sync.write_file("/reset_test.txt", b"data").unwrap();
    sync.flush().unwrap();
    assert!(sync.exists("/reset_test.txt").unwrap());

    sync.reset().unwrap();
    assert!(!sync.exists("/reset_test.txt").unwrap());
}
