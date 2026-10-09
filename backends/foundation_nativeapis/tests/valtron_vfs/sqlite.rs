//! Integration tests: SyncLibsqlDelta through valtron.
//!
//! These tests verify the sync-bridge code path for LibsqlDelta —
//! `SyncLibsqlDelta` delegates to `SyncFs<LibsqlDelta>` which routes all
//! operations through valtron's `exec_async`. Direct async calls on
//! `LibsqlDelta` bypass the bridge.

#![cfg(feature = "vfs-sqlite")]

use std::path::PathBuf;

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem,
};
use foundation_nativeapis::native::vfs::libsql_delta::{LibsqlDelta, SyncLibsqlDelta};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "valtron_libsql_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        // Also remove -journal and -wal files if they exist
        let _ = std::fs::remove_file(format!("{}-journal", self.0.display()));
        let _ = std::fs::remove_file(format!("{}-wal", self.0.display()));
        let _ = std::fs::remove_file(format!("{}-shm", self.0.display()));
    }
}

// ── SyncLibsqlDelta: Basic Operations ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_mkdir_and_stat() {
    let _guard = init_pool();
    let tmp = TempFile::new("mkdir_stat");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

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
fn sync_libsql_create_write_read() {
    let _guard = init_pool();
    let tmp = TempFile::new("create_write_read");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    let file = sync.create("/hello.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();

    let mut buf = vec![0u8; 11];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 11);
    assert_eq!(&buf, b"hello world");

    assert_eq!(file.size().unwrap(), 11);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_read_file_write_file() {
    let _guard = init_pool();
    let tmp = TempFile::new("read_write_file");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

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
fn sync_libsql_open_seekable() {
    let _guard = init_pool();
    let tmp = TempFile::new("open_seekable");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.write_file("/seek.txt", b"0123456789abcdef").unwrap();

    let mut seekable = sync.open_seekable("/seek.txt", OpenMode::Read).unwrap();

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
fn sync_libsql_seek_operations() {
    let _guard = init_pool();
    let tmp = TempFile::new("seek_ops");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

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
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_rename_and_remove() {
    let _guard = init_pool();
    let tmp = TempFile::new("rename_remove");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.write_file("/old.txt", b"content").unwrap();
    sync.rename("/old.txt", "/new.txt").unwrap();

    assert!(!sync.exists("/old.txt").unwrap());
    assert!(sync.exists("/new.txt").unwrap());

    sync.remove("/new.txt").unwrap();
    assert!(!sync.exists("/new.txt").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_remove_all() {
    let _guard = init_pool();
    let tmp = TempFile::new("remove_all");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.mkdir("/a").unwrap();
    sync.write_file("/a/b.txt", b"data").unwrap();
    sync.mkdir("/a/c").unwrap();
    sync.write_file("/a/c/d.txt", b"more").unwrap();

    sync.remove_all("/a").unwrap();
    assert!(!sync.exists("/a").unwrap());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_chmod() {
    let _guard = init_pool();
    let tmp = TempFile::new("chmod");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

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
fn sync_libsql_directory_operations() {
    let _guard = init_pool();
    let tmp = TempFile::new("dir_ops");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.mkdir("/dir").unwrap();
    sync.write_file("/dir/a.txt", b"a").unwrap();
    sync.write_file("/dir/b.txt", b"bb").unwrap();

    let dir = sync.open_directory("/dir").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.name == "a.txt"));
    assert!(entries.iter().any(|e| e.name == "b.txt"));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_copy() {
    let _guard = init_pool();
    let tmp = TempFile::new("copy");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.write_file("/src.txt", b"source data").unwrap();
    sync.copy("/src.txt", "/dst.txt").unwrap();

    assert_eq!(sync.read_file("/src.txt").unwrap(), b"source data");
    assert_eq!(sync.read_file("/dst.txt").unwrap(), b"source data");
}

// ── SyncLibsqlDelta: DeltaStore Operations ──

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_whiteout() {
    let _guard = init_pool();
    let tmp = TempFile::new("whiteout");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.add_whiteout("/hidden.txt", 1).unwrap();
    assert!(sync.is_whiteout("/hidden.txt").unwrap().is_some());
    assert_eq!(sync.is_whiteout("/not_hidden.txt").unwrap(), None);

    sync.remove_whiteout("/hidden.txt").unwrap();
    assert_eq!(sync.is_whiteout("/hidden.txt").unwrap(), None);
}
#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_list_whiteouts() {
    let _guard = init_pool();
    let tmp = TempFile::new("list_whiteouts");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.add_whiteout("/src/a.rs", 1).unwrap();
    sync.add_whiteout("/src/b.rs", 2).unwrap();
    sync.add_whiteout("/lib/c.rs", 3).unwrap();

    let whiteouts = sync.list_whiteouts("/src").unwrap();
    eprintln!("=== LIST WHITEOUTS === {:?}", whiteouts);
    assert_eq!(whiteouts.len(), 2);
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_reset() {
    let _guard = init_pool();
    let tmp = TempFile::new("reset");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.mkdir("/data").unwrap();
    sync.write_file("/data/file.txt", b"data").unwrap();
    sync.add_whiteout("/hidden.txt", 1).unwrap();

    sync.reset().unwrap();

    assert_eq!(sync.is_whiteout("/hidden.txt").unwrap(), None);
    // Root should still exist
    assert!(sync.exists("/").unwrap() || sync.stat("/").is_ok());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_flush() {
    let _guard = init_pool();
    let tmp = TempFile::new("flush");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    sync.flush().unwrap();
}

// ── Large File Test (chunked storage) ──

#[test]
#[foundation_macros::timeout(120_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn sync_libsql_large_file_chunked() {
    let _guard = init_pool();
    let tmp = TempFile::new("large_file");
    let sync = SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap());

    // Write a file larger than the default 64KB chunk size
    let data: Vec<u8> = (0..200_000).map(|i| (i % 256) as u8).collect();
    sync.write_file("/large.bin", &data).unwrap();

    let read = sync.read_file("/large.bin").unwrap();
    assert_eq!(read.len(), data.len());
    assert_eq!(read, data);
}
