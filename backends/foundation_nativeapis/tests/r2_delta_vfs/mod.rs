#![cfg(feature = "vfs-r2")]

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::native::vfs::r2_delta::R2Delta;
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
use foundation_nativeapis::shared::vfs::{DeltaStore, VfsFileSystem};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Check if local CF emulator is available
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

/// Create a SyncFs<R2Delta> pointed at local emulator, or None if unavailable
fn make_sync_r2() -> Option<SyncFs<R2Delta>> {
    if !is_local_cf_available() {
        return None;
    }
    let base = std::env::var("LOCAL_CF_API_BASE").unwrap_or_else(|_| "http://localhost:8789".to_string());
    let bucket = std::env::var("LOCAL_R2_BUCKET").unwrap_or_else(|_| "test-bucket".to_string());
    let delta = R2Delta::new_with_base_url("test-token", "test-account", &bucket, Some(&base)).ok()?;
    Some(SyncFs::new(delta))
}

// ── API Surface Tests (always run, no valtron needed) ──

#[test]
fn test_r2_delta_new_with_credentials() {
    let result = R2Delta::new("test-token", "test-account", "test-bucket");
    assert!(result.is_ok());
}

#[test]
fn test_r2_delta_from_env_no_creds() {
    let result = R2Delta::from_env();
    assert!(result.is_err());
}

#[test]
fn test_r2_delta_is_vfs_file_system() {
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    assert_vfs_fs::<R2Delta>();
}

#[test]
fn test_r2_delta_is_delta_store() {
    fn assert_delta<T: DeltaStore>() {}
    assert_delta::<R2Delta>();
}

// ── Integration Tests through valtron sync bridge ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_r2_kvstore_put_get() {
    let _guard = init_pool();
    let Some(sync) = make_sync_r2() else {
        println!("Skipping R2 test - miniflare not available (set CF_INTEGRATION_TEST=1)");
        return;
    };

    sync.create("/test_kv.txt", 0o644).unwrap();
    sync.write_file("/test_kv.txt", b"Hello, R2!").unwrap();
    let data = sync.read_file("/test_kv.txt").unwrap();
    assert_eq!(&data, b"Hello, R2!");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_r2_exists() {
    let _guard = init_pool();
    let Some(sync) = make_sync_r2() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    sync.create("/exists_test.txt", 0o644).unwrap();
    assert!(sync.exists("/exists_test.txt").unwrap());
    assert!(!sync.exists("/nonexistent.txt").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_r2_whiteout() {
    let _guard = init_pool();
    let Some(sync) = make_sync_r2() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    sync.add_whiteout("/whiteout_test.txt", 1).unwrap();
    assert!(sync.is_whiteout("/whiteout_test.txt").unwrap().is_some());
    sync.remove_whiteout("/whiteout_test.txt").unwrap();
    assert!(sync.is_whiteout("/whiteout_test.txt").unwrap().is_none());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_r2_read_write() {
    let _guard = init_pool();
    let Some(sync) = make_sync_r2() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    sync.create("/rw_test.txt", 0o644).unwrap();
    sync.write_file("/rw_test.txt", b"test content").unwrap();
    let data = sync.read_file("/rw_test.txt").unwrap();
    assert_eq!(&data, b"test content");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_r2_flush_and_reset() {
    let _guard = init_pool();
    let Some(sync) = make_sync_r2() else {
        println!("Skipping R2 test - miniflare not available");
        return;
    };

    sync.create("/reset_test.txt", 0o644).unwrap();
    sync.write_file("/reset_test.txt", b"data").unwrap();
    sync.flush().unwrap();
    assert!(sync.exists("/reset_test.txt").unwrap());

    sync.reset().unwrap();
    assert!(!sync.exists("/reset_test.txt").unwrap());
}
