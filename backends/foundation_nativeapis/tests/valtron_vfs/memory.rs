//! Integration tests: SyncFs<MemoryFs> and SyncFs<MemoryDelta> through valtron.
//!
//! These tests verify the sync-bridge code path — all `SyncFs<A>` methods
//! call `exec_async` which routes through valtron's executor. Direct sync
//! calls on `MemoryFs` bypass the bridge entirely.

#![cfg(feature = "vfs")]

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, MemoryDelta, MemoryFs, OpenMode, SeekableVfsFile, VfsDirectory, VfsFile,
    VfsFileSystem,
};
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

// ── SyncFs<MemoryFs>: Basic Operations ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_mkdir_and_stat() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/src").unwrap();
    sync.mkdir("/src/lib").unwrap();

    let meta = sync.stat("/src").unwrap();
    assert!(meta.size == 0);

    assert!(sync.exists("/src").unwrap());
    assert!(sync.exists("/src/lib").unwrap());
    assert!(!sync.exists("/src/lib/nope").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_create_write_read() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/data").unwrap();
    let file = sync.create("/data/hello.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();

    let mut buf = vec![0u8; 11];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 11);
    assert_eq!(&buf, b"hello world");

    // Size reflects written data
    assert_eq!(file.size().unwrap(), 11);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_read_file_write_file() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/test.txt", b"test content").unwrap();
    let data = sync.read_file("/test.txt").unwrap();
    assert_eq!(&data, b"test content");

    // Overwrite
    sync.write_file("/test.txt", b"new content").unwrap();
    let data = sync.read_file("/test.txt").unwrap();
    assert_eq!(&data, b"new content");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_open_seekable() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/seek.txt", b"0123456789abcdef").unwrap();

    let mut seekable = sync.open_seekable("/seek.txt", OpenMode::Read).unwrap();

    // Read sequentially via cursor
    let mut buf = vec![0u8; 4];
    let n = seekable.read(&mut buf).unwrap();
    assert_eq!(n, 4);
    assert_eq!(&buf, b"0123");
    assert_eq!(seekable.position(), 4);

    let n = seekable.read(&mut buf).unwrap();
    assert_eq!(n, 4);
    assert_eq!(&buf, b"4567");
    assert_eq!(seekable.position(), 8);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_seek_operations() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/seek.txt", b"0123456789").unwrap();
    let mut seekable = sync.open_seekable("/seek.txt", OpenMode::Read).unwrap();

    // SeekFrom::Start
    let pos = seekable.seek(std::io::SeekFrom::Start(5)).unwrap();
    assert_eq!(pos, 5);

    // SeekFrom::Current
    let pos = seekable.seek(std::io::SeekFrom::Current(2)).unwrap();
    assert_eq!(pos, 7);

    // SeekFrom::End
    let pos = seekable.seek(std::io::SeekFrom::End(-3)).unwrap();
    assert_eq!(pos, 7);

    // SeekFrom::End positive
    let pos = seekable.seek(std::io::SeekFrom::End(5)).unwrap();
    assert_eq!(pos, 15);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_rename_and_remove() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/old.txt", b"content").unwrap();
    sync.rename("/old.txt", "/new.txt").unwrap();

    assert!(!sync.exists("/old.txt").unwrap());
    assert!(sync.exists("/new.txt").unwrap());
    assert_eq!(sync.read_file("/new.txt").unwrap(), b"content");

    sync.remove("/new.txt").unwrap();
    assert!(!sync.exists("/new.txt").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_remove_all() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/a").unwrap();
    sync.write_file("/a/b.txt", b"data").unwrap();
    sync.mkdir("/a/c").unwrap();
    sync.write_file("/a/c/d.txt", b"more").unwrap();

    sync.remove_all("/a").unwrap();
    assert!(!sync.exists("/a").unwrap());
    assert!(!sync.exists("/a/b.txt").unwrap());
    assert!(!sync.exists("/a/c").unwrap());
    assert!(!sync.exists("/a/c/d.txt").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_chmod() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/file.txt", b"data").unwrap();
    let before = sync.stat("/file.txt").unwrap();
    assert_eq!(before.permissions, 0o644);

    sync.chmod("/file.txt", 0o755).unwrap();
    let after = sync.stat("/file.txt").unwrap();
    assert_eq!(after.permissions, 0o755);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_directory_operations() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/dir").unwrap();
    sync.write_file("/dir/a.txt", b"a").unwrap();
    sync.write_file("/dir/b.txt", b"bb").unwrap();

    let dir = sync.open_directory("/dir").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 2);

    let meta = dir.stat("a.txt").unwrap();
    assert_eq!(meta.size, 1);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_copy() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/src.txt", b"source data").unwrap();
    sync.copy("/src.txt", "/dst.txt").unwrap();

    assert_eq!(sync.read_file("/src.txt").unwrap(), b"source data");
    assert_eq!(sync.read_file("/dst.txt").unwrap(), b"source data");
}

// ── SyncFs<MemoryDelta>: DeltaStore Operations ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_delta_basic() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryDelta::new());

    sync.mkdir("/delta").unwrap();
    sync.write_file("/delta/file.txt", b"delta content").unwrap();
    assert_eq!(sync.read_file("/delta/file.txt").unwrap(), b"delta content");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_delta_whiteout() {
    let _guard = init_pool();
    let sync: SyncFs<MemoryDelta> = SyncFs::new(MemoryDelta::new());

    sync.add_whiteout("/hidden.txt", 1).unwrap();
    assert_eq!(sync.is_whiteout("/hidden.txt").unwrap(), Some(1));
    assert_eq!(sync.is_whiteout("/not_hidden.txt").unwrap(), None);

    sync.remove_whiteout("/hidden.txt").unwrap();
    assert_eq!(sync.is_whiteout("/hidden.txt").unwrap(), None);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_delta_list_whiteouts() {
    let _guard = init_pool();
    let sync: SyncFs<MemoryDelta> = SyncFs::new(MemoryDelta::new());

    sync.add_whiteout("/src/a.rs", 1).unwrap();
    sync.add_whiteout("/src/b.rs", 2).unwrap();
    sync.add_whiteout("/lib/c.rs", 3).unwrap();

    let whiteouts = sync.list_whiteouts("/src").unwrap();
    assert_eq!(whiteouts.len(), 2);
    assert_eq!(whiteouts[0].0, "/src/a.rs");
    assert_eq!(whiteouts[1].0, "/src/b.rs");
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_delta_reset() {
    let _guard = init_pool();
    let sync: SyncFs<MemoryDelta> = SyncFs::new(MemoryDelta::new());

    sync.mkdir("/data").unwrap();
    sync.write_file("/data/file.txt", b"data").unwrap();
    sync.add_whiteout("/hidden.txt", 1).unwrap();

    sync.reset().unwrap();

    assert_eq!(sync.is_whiteout("/hidden.txt").unwrap(), None);
    assert!(!sync.exists("/data/file.txt").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_delta_flush() {
    let _guard = init_pool();
    let sync: SyncFs<MemoryDelta> = SyncFs::new(MemoryDelta::new());

    // Flush should be a no-op for MemoryDelta but must not panic
    sync.flush().unwrap();
}

// ── Concurrent Operations via Valtron Threads ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_concurrent_mkdir() {
    let _guard = init_pool();

    let sync = std::sync::Arc::new(SyncFs::new(MemoryFs::new()));

    let mut handles = Vec::new();
    for i in 0..5 {
        let s = sync.clone();
        handles.push(std::thread::spawn(move || {
            let path = format!("/dir_{i}");
            let result = s.mkdir(&path);
            result.map(|_| path)
        }));
    }

    for handle in handles {
        let path = handle.join().unwrap().unwrap();
        assert!(sync.exists(&path).unwrap());
    }
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn syncfs_memory_concurrent_write_different_files() {
    let _guard = init_pool();

    let sync = std::sync::Arc::new(SyncFs::new(MemoryFs::new()));

    let mut handles = Vec::new();
    for i in 0..5 {
        let s = sync.clone();
        handles.push(std::thread::spawn(move || {
            let path = format!("/file_{i}.txt");
            let content = format!("content {i}").into_bytes();
            s.write_file(&path, &content).unwrap();
            let read = s.read_file(&path).unwrap();
            assert_eq!(read, content);
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
