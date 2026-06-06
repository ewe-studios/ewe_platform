//! Integration tests: AsyncVfsFileSystem traits called through valtron futures.
//!
//! These tests verify that async trait implementations work correctly when
//! executed through valtron's `execute` + `collect_one` — the same code path
//! that `exec_async` uses internally.

#![cfg(feature = "vfs")]

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::async_traits::{
    AsyncDeltaStore, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use foundation_nativeapis::shared::vfs::error::VfsResult;
use foundation_nativeapis::shared::vfs::{MemoryDelta, MemoryFs, OpenMode};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Helper: run an async future through valtron and return its result.
fn run_async<T: Send + 'static, F>(future: F) -> VfsResult<T>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("execute failed");
    let result: Option<Result<T, foundation_errstacks::ErrorTrace<_>>> = collect_one(stream);
    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => Err(foundation_errstacks::ErrorTrace::new(
            foundation_nativeapis::shared::vfs::VfsError::Backend {
                message: "no result from valtron".into(),
            },
        )),
    }
}

// ── AsyncVfsFileSystem via Valtron ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memoryfs_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryFs> = Arc::new(MemoryFs::new());

    // mkdir_async
    let fs_clone = fs.clone();
    run_async(async move {
        fs_clone.mkdir_async("/src".to_string()).await
    }).unwrap();

    // stat_async
    let fs_clone = fs.clone();
    let meta = run_async(async move {
        fs_clone.stat_async("/src".to_string()).await
    }).unwrap();
    assert!(meta.size == 0);

    // exists_async
    let fs_clone = fs.clone();
    let exists = run_async(async move {
        fs_clone.exists_async("/src".to_string()).await
    }).unwrap();
    assert!(exists);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memoryfs_write_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryFs> = Arc::new(MemoryFs::new());

    // create + write via valtron
    let fs_clone = fs.clone();
    let file = run_async(async move {
        fs_clone.create_async("/test.txt".to_string(), 0o644).await
    }).unwrap();
    let file = Arc::new(file);

    let f = file.clone();
    run_async(async move {
        f.write_at_async(b"hello".to_vec(), 0).await
    }).unwrap();

    // read via valtron
    let f = file.clone();
    let data = run_async(async move {
        f.read_at_async(5, 0).await
    }).unwrap();
    assert_eq!(&data, b"hello");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memoryfs_open_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryFs> = Arc::new(MemoryFs::new());

    let fs_setup = fs.clone();
    run_async(async move {
        fs_setup.write_file_async("/open.txt".to_string(), b"open me".to_vec()).await
    }).unwrap();

    // open_async via valtron
    let fs_clone = fs.clone();
    let file = run_async(async move {
        fs_clone.open_async("/open.txt".to_string(), OpenMode::Read).await
    }).unwrap();

    let f = Arc::new(file);
    let data = run_async(async move {
        f.read_at_async(7, 0).await
    }).unwrap();
    assert_eq!(&data, b"open me");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memoryfs_directory_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryFs> = Arc::new(MemoryFs::new());

    let fs_s = fs.clone();
    run_async(async move { fs_s.mkdir_async("/dir".to_string()).await }).unwrap();
    let fs_s = fs.clone();
    run_async(async move { fs_s.write_file_async("/dir/a.txt".to_string(), b"a".to_vec()).await }).unwrap();
    let fs_s = fs.clone();
    run_async(async move { fs_s.write_file_async("/dir/b.txt".to_string(), b"bb".to_vec()).await }).unwrap();

    let fs_clone = fs.clone();
    let dir = run_async(async move {
        fs_clone.open_directory_async("/dir".to_string()).await
    }).unwrap();

    let entries = run_async(async move {
        dir.list_async().await
    }).unwrap();
    assert_eq!(entries.len(), 2);
}

// ── AsyncDeltaStore via Valtron ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memorydelta_whiteout_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryDelta> = Arc::new(MemoryDelta::new());

    let fs_clone = fs.clone();
    run_async(async move {
        fs_clone.add_whiteout_async("/hidden.txt".to_string(), 1).await
    }).unwrap();

    let fs_clone = fs.clone();
    let version = run_async(async move {
        fs_clone.is_whiteout_async("/hidden.txt".to_string()).await
    }).unwrap();
    assert_eq!(version, Some(1));

    let fs_clone = fs.clone();
    run_async(async move {
        fs_clone.remove_whiteout_async("/hidden.txt".to_string()).await
    }).unwrap();

    let fs_clone = fs.clone();
    let version = run_async(async move {
        fs_clone.is_whiteout_async("/hidden.txt".to_string()).await
    }).unwrap();
    assert_eq!(version, None);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memorydelta_reset_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryDelta> = Arc::new(MemoryDelta::new());

    let fs_s = fs.clone();
    run_async(async move { fs_s.mkdir_async("/data".to_string()).await }).unwrap();
    let fs_s = fs.clone();
    run_async(async move { fs_s.write_file_async("/data/file.txt".to_string(), b"data".to_vec()).await }).unwrap();

    let fs_clone = fs.clone();
    run_async(async move {
        fs_clone.add_whiteout_async("/hidden.txt".to_string(), 1).await
    }).unwrap();

    // Reset via valtron
    let fs_clone = fs.clone();
    run_async(async move {
        fs_clone.reset_async().await
    }).unwrap();

    let fs_clone = fs.clone();
    let version = run_async(async move {
        fs_clone.is_whiteout_async("/hidden.txt".to_string()).await
    }).unwrap();
    assert_eq!(version, None);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn async_memorydelta_flush_via_valtron() {
    let _guard = init_pool();
    let fs: Arc<MemoryDelta> = Arc::new(MemoryDelta::new());

    let fs_clone = fs.clone();
    run_async(async move {
        fs_clone.flush_async().await
    }).unwrap();
}
