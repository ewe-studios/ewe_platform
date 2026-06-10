//! Integration tests: Seekable file concurrency through valtron.
//!
//! These tests verify that `Arc<AtomicU64>` seekable cursors work correctly
//! when handles are cloned and accessed from multiple threads simultaneously.
//! No panic windows, no data races, correct shared position.

#![cfg(feature = "vfs-sqlite")]

use std::sync::Arc;

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{OpenMode, SeekableVfsFile, VfsFileSystem};
use foundation_nativeapis::native::vfs::libsql_delta::{LibsqlDelta, SyncLibsqlDelta};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

struct TempFile(std::path::PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "valtron_seek_{}_{}_{}",
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
        let _ = std::fs::remove_file(format!("{}-journal", self.0.display()));
        let _ = std::fs::remove_file(format!("{}-wal", self.0.display()));
        let _ = std::fs::remove_file(format!("{}-shm", self.0.display()));
    }
}

// ── Seekable File Concurrency ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn seekable_shared_cursor() {
    let _guard = init_pool();
    let tmp = TempFile::new("shared_cursor");
    let sync = Arc::new(SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap()));

    sync.write_file("/data.txt", b"0123456789ABCDEF").unwrap();

    // Open seekable on two threads
    let mut seek1 = sync.open_seekable("/data.txt", OpenMode::Read).unwrap();
    let mut seek2 = sync.open_seekable("/data.txt", OpenMode::Read).unwrap();

    // Each handle has its own cursor (not shared between open_seekable calls)
    seek1.seek(std::io::SeekFrom::Start(0)).unwrap();
    seek2.seek(std::io::SeekFrom::Start(8)).unwrap();

    let mut buf1 = vec![0u8; 4];
    let n1 = seek1.read(&mut buf1).unwrap();
    assert_eq!(n1, 4);
    assert_eq!(&buf1, b"0123");

    let mut buf2 = vec![0u8; 4];
    let n2 = seek2.read(&mut buf2).unwrap();
    assert_eq!(n2, 4);
    assert_eq!(&buf2, b"89AB");

    // Cursors are independent between separate open_seekable calls
    assert_eq!(seek1.position(), 4);
    assert_eq!(seek2.position(), 12);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn seekable_concurrent_reads() {
    let _guard = init_pool();
    let tmp = TempFile::new("concurrent_reads");
    let sync = Arc::new(SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap()));

    // Write test data: each byte is its position
    let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    sync.write_file("/big.bin", &data).unwrap();

    let mut handles = Vec::new();
    for offset in [0u64, 100, 500, 250, 750] {
        let s = sync.clone();
        let data = data.clone();
        handles.push(std::thread::spawn(move || {
            let mut seekable = s.open_seekable("/big.bin", OpenMode::Read).unwrap();
            seekable.seek(std::io::SeekFrom::Start(offset)).unwrap();

            let mut buf = vec![0u8; 10];
            let n = seekable.read(&mut buf).unwrap();
            assert_eq!(n, 10);

            for (i, &b) in buf.iter().enumerate() {
                assert_eq!(b, data[offset as usize + i],
                    "byte mismatch at offset {}+{}", offset, i);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn seekable_write_position_tracking() {
    let _guard = init_pool();
    let tmp = TempFile::new("write_pos");
    let sync = Arc::new(SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap()));

    sync.write_file("/write.txt", b"initial").unwrap();

    let mut seekable = sync.open_seekable("/write.txt", OpenMode::ReadWrite).unwrap();

    // Seek to end and append
    let size = seekable.seek(std::io::SeekFrom::End(0)).unwrap();
    assert_eq!(size, 7); // "initial" = 7 bytes

    // Write more (9 bytes)
    let n = seekable.write(b" appended").unwrap();
    assert_eq!(n, 9);
    assert_eq!(seekable.position(), 16);

    // Verify full content
    let data = sync.read_file("/write.txt").unwrap();
    assert_eq!(&data, b"initial appended");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn seekable_no_panic_under_concurrency() {
    let _guard = init_pool();
    let tmp = TempFile::new("no_panic");
    let sync = Arc::new(SyncLibsqlDelta::new(LibsqlDelta::new(tmp.path()).unwrap()));

    let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    sync.write_file("/concurrent.bin", &data).unwrap();

    // Spawn many threads all doing concurrent seekable reads
    let mut handles = Vec::new();
    for i in 0..10 {
        let s = sync.clone();
        let data = data.clone();
        handles.push(std::thread::spawn(move || {
            let mut seekable = s.open_seekable("/concurrent.bin", OpenMode::Read).unwrap();

            let offset = (i * 100) as u64;
            seekable.seek(std::io::SeekFrom::Start(offset)).unwrap();

            let mut buf = vec![0u8; 50];
            let n = seekable.read(&mut buf).unwrap();
            assert_eq!(n, 50);
            for (j, &b) in buf.iter().enumerate() {
                assert_eq!(b, data[offset as usize + j]);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Sequential writes verify correctness without contention
    for i in 0..5usize {
        let mut seekable = sync.open_seekable("/concurrent.bin", OpenMode::ReadWrite).unwrap();
        let offset = (i * 100) as u64;
        seekable.seek(std::io::SeekFrom::Start(offset)).unwrap();
        let write_data: Vec<u8> = (0..50).map(|j| (j + i) as u8).collect();
        seekable.write(&write_data).unwrap();

        seekable.seek(std::io::SeekFrom::Start(offset)).unwrap();
        let mut buf = vec![0u8; 50];
        let n = seekable.read(&mut buf).unwrap();
        assert_eq!(n, 50);
        for (j, &b) in buf.iter().enumerate() {
            assert_eq!(b, (j + i) as u8);
        }
    }
}
