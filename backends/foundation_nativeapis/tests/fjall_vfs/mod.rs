#![cfg(feature = "vfs-fjall")]

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::shared::vfs::fjall_fs::{InodeDelta, InodeFs};
use foundation_nativeapis::shared::vfs::{
    AsyncDeltaStore, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem, DeltaStore, OpenMode,
    SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem, VfsFileType,
};

// ── InodeFs: File Operations ──

#[test]
fn test_fjall_create_and_read_file() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/hello.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();
    let mut buf = vec![0u8; 11];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 11);
    assert_eq!(&buf, b"hello world");
}

#[test]
fn test_fjall_create_file_in_nonexistent_dir() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let result = fs.create("/no/such/dir/file.txt", 0o644);
    assert!(result.is_err());
}

#[test]
fn test_fjall_open_nonexistent_file() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let result = fs.open("/nope.txt", OpenMode::Read);
    assert!(result.is_err());
}

#[test]
fn test_fjall_write_to_read_only_file() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/readonly.txt", 0o644).unwrap();
    file.write_at(b"initial", 0).unwrap();
    let ro = fs.open("/readonly.txt", OpenMode::Read).unwrap();
    let result = ro.write_at(b"nope", 0);
    assert!(result.is_err());
}

#[test]
fn test_fjall_file_read_at_offset() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/data.bin", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();
    let mut buf = [0u8; 3];
    let n = file.read_at(&mut buf, 5).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"FGH");
}

#[test]
fn test_fjall_file_write_at_offset() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/sparse.bin", 0o644).unwrap();
    file.write_at(b"XYZ", 10).unwrap();
    assert_eq!(file.size().unwrap(), 13);
    let mut buf = [0u8; 13];
    file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf[..10], &[0u8; 10]);
    assert_eq!(&buf[10..], b"XYZ");
}

#[test]
fn test_fjall_file_truncate() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/trunc.txt", 0o644).unwrap();
    file.write_at(b"123456789", 0).unwrap();
    file.truncate(3).unwrap();
    assert_eq!(file.size().unwrap(), 3);
    let mut buf = [0u8; 3];
    file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf, b"123");
}

#[test]
fn test_fjall_seekable_sequential_read() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/seq.txt", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();
    let mut sf = fs.open_seekable("/seq.txt", OpenMode::Read).unwrap();
    let mut buf = vec![0u8; 5];
    let n = sf.read(&mut buf).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf[..n], b"ABCDE");
}

#[test]
fn test_fjall_seekable_seek_and_read() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/seek.txt", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();
    let mut sf = fs.open_seekable("/seek.txt", OpenMode::Read).unwrap();
    sf.seek(std::io::SeekFrom::Start(5)).unwrap();
    let mut buf = vec![0u8; 3];
    sf.read(&mut buf).unwrap();
    assert_eq!(&buf, b"FGH");
}

// ── InodeFs: Directory Operations ──

#[test]
fn test_fjall_mkdir_and_list() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.mkdir("/src").unwrap();
    fs.create("/src/main.rs", 0o644).unwrap();
    let dir = fs.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    assert!(entries.iter().any(|e| e.name == "src"));
}

#[test]
fn test_fjall_mkdir_nested() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.mkdir("/a/b/c").unwrap();
    assert!(fs.exists("/a/b/c").unwrap());
}

#[test]
fn test_fjall_remove_empty_dir() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.mkdir("/empty").unwrap();
    fs.remove("/empty").unwrap();
    assert!(!fs.exists("/empty").unwrap());
}

#[test]
fn test_fjall_readdir_mixed() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.mkdir("/dir").unwrap();
    fs.create("/dir/file.txt", 0o644).unwrap();
    fs.mkdir("/dir/sub").unwrap();
    let dir = fs.open_directory("/dir").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_fjall_stat_returns_correct_metadata() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/stat.txt", 0o644).unwrap();
    file.write_at(b"hello", 0).unwrap();
    let meta = fs.stat("/stat.txt").unwrap();
    assert_eq!(meta.size, 5);
    assert_eq!(meta.file_type, VfsFileType::Regular);
}

#[test]
fn test_fjall_chmod_updates_permissions() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.create("/perm.txt", 0o644).unwrap();
    fs.chmod("/perm.txt", 0o755).unwrap();
    let meta = fs.stat("/perm.txt").unwrap();
    assert_eq!(meta.permissions, 0o755);
}

#[test]
fn test_fjall_symlink_create_and_readlink() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.symlink("/target", "/link").unwrap();
    let target = fs.readlink("/link").unwrap();
    assert_eq!(target, "/target");
}

// ── InodeFs: Convenience Operations ──

#[test]
fn test_fjall_read_file_convenience() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    let file = fs.create("/rf.txt", 0o644).unwrap();
    file.write_at(b"data", 0).unwrap();
    let data = fs.read_file("/rf.txt").unwrap();
    assert_eq!(&data, b"data");
}

#[test]
fn test_fjall_write_file_convenience() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.create("/wf.txt", 0o644).unwrap();
    fs.write_file("/wf.txt", b"written").unwrap();
    let data = fs.read_file("/wf.txt").unwrap();
    assert_eq!(&data, b"written");
}

#[test]
fn test_fjall_mkdir_all() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.mkdir_all("/a/b/c/d").unwrap();
    assert!(fs.exists("/a/b/c/d").unwrap());
}

#[test]
fn test_fjall_remove_all() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.mkdir_all("/parent/child").unwrap();
    fs.create("/parent/child/file.txt", 0o644).unwrap();
    fs.remove_all("/parent").unwrap();
    assert!(!fs.exists("/parent").unwrap());
}

// ── InodeDelta: Whiteout Operations ──

#[test]
fn test_fjall_whiteout_add_and_check() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    delta.add_whiteout("/hidden.txt", 1).unwrap();
    assert!(delta.is_whiteout("/hidden.txt").unwrap().is_some());
}

#[test]
fn test_fjall_whiteout_remove() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    delta.add_whiteout("/gone.txt", 1).unwrap();
    delta.remove_whiteout("/gone.txt").unwrap();
    assert!(delta.is_whiteout("/gone.txt").unwrap().is_none());
}

#[test]
fn test_fjall_whiteout_list() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    delta.add_whiteout("/dir/a.txt", 1).unwrap();
    delta.add_whiteout("/dir/b.txt", 2).unwrap();
    delta.add_whiteout("/other.txt", 3).unwrap();
    let whiteouts = delta.list_whiteouts("/dir").unwrap();
    assert_eq!(whiteouts.len(), 2);
}

#[test]
fn test_fjall_whiteout_reset_clears_all() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    delta.add_whiteout("/a.txt", 1).unwrap();
    delta.add_whiteout("/b.txt", 2).unwrap();
    delta.reset().unwrap();
    assert!(delta.list_whiteouts("/").unwrap().is_empty());
}

#[test]
fn test_fjall_delta_create_and_read() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    delta.create("/delta.txt", 0o644).unwrap();
    let file = delta.open("/delta.txt", OpenMode::Write).unwrap();
    file.write_at(b"delta content", 0).unwrap();
    let data = delta.read_file("/delta.txt").unwrap();
    assert_eq!(&data, b"delta content");
}

#[test]
fn test_fjall_delta_exists_respects_whiteout() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    delta.create("/visible.txt", 0o644).unwrap();
    assert!(delta.exists("/visible.txt").unwrap());
    delta.add_whiteout("/visible.txt", 1).unwrap();
    assert!(!delta.exists("/visible.txt").unwrap());
}

// ── InodeFs: Inode Operations ──

#[test]
fn test_fjall_inode() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.create("/inode.txt", 0o644).unwrap();
    fs.write_file("/inode.txt", b"data").unwrap();
    let ino = fs.inode("/inode.txt").unwrap();
    assert!(ino > 0);
}

#[test]
fn test_fjall_path_by_inode() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.create("/path_lookup.txt", 0o644).unwrap();
    fs.write_file("/path_lookup.txt", b"data").unwrap();
    let ino = fs.inode("/path_lookup.txt").unwrap();
    let path = fs.path_by_inode(ino).unwrap();
    assert_eq!(path, "/path_lookup.txt");
}

#[test]
fn test_fjall_stat_by_inode() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));
    fs.create("/stat_ino.txt", 0o644).unwrap();
    fs.write_file("/stat_ino.txt", b"data").unwrap();
    let ino = fs.inode("/stat_ino.txt").unwrap();
    let meta = fs.stat_by_inode(ino).unwrap();
    assert_eq!(meta.size, 4);
}

// ── InodeFs: Async Contract ──

#[valtron_test]
async fn async_inodefs_mkdir_stat_exists() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));

    fs.mkdir_async("/src".to_string()).await.unwrap();
    let meta = fs.stat_async("/src".to_string()).await.unwrap();
    assert_eq!(meta.size, 0);
    assert!(fs.exists_async("/src".to_string()).await.unwrap());
}

#[valtron_test]
async fn async_inodefs_create_write_read() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));

    let file = fs
        .create_async("/test.txt".to_string(), 0o644)
        .await
        .unwrap();
    file.write_at_async(b"hello".to_vec(), 0).await.unwrap();
    let data = file.read_at_async(5, 0).await.unwrap();
    assert_eq!(&data, b"hello");
}

#[valtron_test]
async fn async_inodefs_open_read() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));

    fs.write_file_async("/open.txt".to_string(), b"open me".to_vec())
        .await
        .unwrap();
    let file = fs
        .open_async("/open.txt".to_string(), OpenMode::Read)
        .await
        .unwrap();
    let data = file.read_at_async(7, 0).await.unwrap();
    assert_eq!(&data, b"open me");
}

#[valtron_test]
async fn async_inodefs_directory_list() {
    let fs = InodeFs::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));

    fs.mkdir_async("/dir".to_string()).await.unwrap();
    fs.write_file_async("/dir/a.txt".to_string(), b"a".to_vec())
        .await
        .unwrap();
    fs.write_file_async("/dir/b.txt".to_string(), b"bb".to_vec())
        .await
        .unwrap();

    let dir = fs.open_directory_async("/dir".to_string()).await.unwrap();
    let entries = dir.list_async().await.unwrap();
    assert_eq!(entries.len(), 2);
}

#[valtron_test]
async fn async_inodedelta_whiteouts() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));

    delta
        .add_whiteout_async("/hidden.txt".to_string(), 1)
        .await
        .unwrap();
    assert_eq!(
        delta
            .is_whiteout_async("/hidden.txt".to_string())
            .await
            .unwrap(),
        Some(1)
    );

    delta
        .remove_whiteout_async("/hidden.txt".to_string())
        .await
        .unwrap();
    assert_eq!(
        delta
            .is_whiteout_async("/hidden.txt".to_string())
            .await
            .unwrap(),
        None
    );
}

#[valtron_test]
async fn async_inodedelta_flush_and_reset() {
    let delta = InodeDelta::new(foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    )));

    delta
        .add_whiteout_async("/hidden.txt".to_string(), 1)
        .await
        .unwrap();
    delta.flush_async().await.unwrap();
    delta.reset_async().await.unwrap();
    assert_eq!(
        delta
            .is_whiteout_async("/hidden.txt".to_string())
            .await
            .unwrap(),
        None
    );
}
