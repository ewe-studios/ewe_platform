#![cfg(feature = "vfs-turso")]
#![recursion_limit = "256"]

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::native::vfs::turso_delta::TursoDelta;
use foundation_nativeapis::shared::vfs::async_traits::{AsyncDeltaStore, AsyncVfsFileSystem};
use foundation_nativeapis::shared::vfs::exec_async::exec_async;
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
use foundation_nativeapis::shared::vfs::{DeltaStore, VfsFileSystem};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Create a `SyncFs<TursoDelta>` over an in-memory database.
fn make_sync_turso() -> SyncFs<TursoDelta> {
    let delta = exec_async(async move {
        TursoDelta::in_memory(std::sync::Arc::new(
            foundation_core::valtron::MonotonicClock::new(),
        ))
        .await
    })
    .unwrap();
    SyncFs::new(delta)
}

// ── API Surface Tests (always run, no valtron needed) ──

#[test]
fn test_turso_delta_is_vfs_file_system() {
    // The delta is async; sync callers go through `SyncFs`.
    fn assert_async_fs<T: AsyncVfsFileSystem>() {}
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    assert_async_fs::<TursoDelta>();
    assert_vfs_fs::<SyncFs<TursoDelta>>();
}

#[test]
fn test_turso_delta_is_delta_store() {
    fn assert_async_delta<T: AsyncDeltaStore>() {}
    fn assert_delta<T: DeltaStore>() {}
    assert_async_delta::<TursoDelta>();
    assert_delta::<SyncFs<TursoDelta>>();
}

// ── Integration Tests through valtron sync bridge ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_turso_kvstore_put_get() {
    let _guard = init_pool();
    let sync = make_sync_turso();

    sync.create("/test_kv.txt", 0o644).unwrap();
    sync.write_file("/test_kv.txt", b"Hello, Turso!").unwrap();
    let data = sync.read_file("/test_kv.txt").unwrap();
    assert_eq!(&data, b"Hello, Turso!");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_turso_exists() {
    let _guard = init_pool();
    let sync = make_sync_turso();

    sync.create("/exists_test.txt", 0o644).unwrap();
    assert!(sync.exists("/exists_test.txt").unwrap());
    assert!(!sync.exists("/nonexistent.txt").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_turso_whiteout() {
    let _guard = init_pool();
    let sync = make_sync_turso();

    sync.add_whiteout("/whiteout_test.txt", 1).unwrap();
    assert!(sync.is_whiteout("/whiteout_test.txt").unwrap().is_some());
    sync.remove_whiteout("/whiteout_test.txt").unwrap();
    assert!(sync.is_whiteout("/whiteout_test.txt").unwrap().is_none());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_turso_read_write() {
    let _guard = init_pool();
    let sync = make_sync_turso();

    sync.create("/rw_test.txt", 0o644).unwrap();
    sync.write_file("/rw_test.txt", b"test content").unwrap();
    let data = sync.read_file("/rw_test.txt").unwrap();
    assert_eq!(&data, b"test content");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_turso_flush_and_reset() {
    let _guard = init_pool();
    let sync = make_sync_turso();

    sync.create("/reset_test.txt", 0o644).unwrap();
    sync.write_file("/reset_test.txt", b"data").unwrap();
    sync.flush().unwrap();
    assert!(sync.exists("/reset_test.txt").unwrap());

    sync.reset().unwrap();
    assert!(!sync.exists("/reset_test.txt").unwrap());
}

// ── Local file engine test (the E4 distinguishing behavior) ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_turso_file_persistence() {
    let _guard = init_pool();
    let path = std::env::temp_dir().join(format!(
        "vfs-turso-test-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&path);

    {
        let db_path = path.clone();
        let delta = exec_async(async move {
            TursoDelta::new(
                db_path,
                std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
            )
            .await
        })
        .unwrap();
        let sync = SyncFs::new(delta);
        sync.create("/persist.txt", 0o644).unwrap();
        sync.write_file("/persist.txt", b"persisted").unwrap();
        sync.flush().unwrap();
    }

    {
        let db_path = path.clone();
        let delta = exec_async(async move {
            TursoDelta::new(
                db_path,
                std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
            )
            .await
        })
        .unwrap();
        let sync = SyncFs::new(delta);
        assert_eq!(sync.read_file("/persist.txt").unwrap(), b"persisted");
    }

    let _ = std::fs::remove_file(&path);
}
