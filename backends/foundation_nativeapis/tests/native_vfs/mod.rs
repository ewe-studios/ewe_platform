#![cfg(feature = "vfs-native")]

use foundation_nativeapis::native::vfs::NativeFs;
use foundation_core::valtron::valtron_test;
use foundation_nativeapis::shared::vfs::{AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem, SyncFs};
use foundation_nativeapis::shared::vfs::{
    OpenMode, SeekFrom, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem, VfsFileType,
};
use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vfs_native_test_{name}_{}_{}", std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_read_existing_file() {
    let tmp = TempDir::new("read");
    fs::write(tmp.path().join("hello.txt"), b"hello world").unwrap();

    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    let data = native_fs.read_file("/hello.txt").unwrap();
    assert_eq!(data, b"hello world");
}

#[test]
fn test_write_and_read_back() {
    let tmp = TempDir::new("write");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());

    native_fs.write_file("/output.txt", b"written by vfs").unwrap();

    let on_disk = fs::read(tmp.path().join("output.txt")).unwrap();
    assert_eq!(on_disk, b"written by vfs");

    let via_vfs = native_fs.read_file("/output.txt").unwrap();
    assert_eq!(via_vfs, b"written by vfs");
}

#[test]
fn test_path_containment_rejects_traversal() {
    let tmp = TempDir::new("contain");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());

    let result = native_fs.read_file("/../../../etc/passwd");
    assert!(result.is_err(), "path traversal should be rejected");
}

#[test]
#[cfg(unix)]
fn test_path_containment_rejects_symlink_escape() {
    let tmp = TempDir::new("symlink_escape");
    std::os::unix::fs::symlink("/etc", tmp.path().join("escape")).unwrap();

    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    let result = native_fs.read_file("/escape/passwd");
    assert!(result.is_err(), "symlink escape should be rejected");
}

#[test]
fn test_stat_returns_correct_metadata() {
    let tmp = TempDir::new("stat");
    fs::write(tmp.path().join("data.bin"), b"12345").unwrap();

    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    let meta = native_fs.stat("/data.bin").unwrap();
    assert_eq!(meta.size, 5);
    assert_eq!(meta.file_type, VfsFileType::Regular);
    assert!(meta.modified.is_some());
}

#[test]
fn test_mkdir_and_list() {
    let tmp = TempDir::new("mkdir");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());

    native_fs.mkdir("/subdir").unwrap();
    native_fs.write_file("/subdir/file.txt", b"in subdir").unwrap();

    let dir = native_fs.open_directory("/subdir").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "file.txt");
}

#[test]
fn test_rename_file() {
    let tmp = TempDir::new("rename");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());

    native_fs.write_file("/old.txt", b"data").unwrap();
    native_fs.rename("/old.txt", "/new.txt").unwrap();

    assert!(!native_fs.exists("/old.txt").unwrap());
    assert_eq!(native_fs.read_file("/new.txt").unwrap(), b"data");
}

#[test]
fn test_remove_file() {
    let tmp = TempDir::new("remove");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());

    native_fs.write_file("/gone.txt", b"bye").unwrap();
    native_fs.remove("/gone.txt").unwrap();
    assert!(!native_fs.exists("/gone.txt").unwrap());
}

#[test]
fn test_seekable_read_write() {
    let tmp = TempDir::new("seekable");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    native_fs.write_file("/seek.txt", b"ABCDEFGHIJ").unwrap();

    let mut sf = native_fs
        .open_seekable("/seek.txt", OpenMode::ReadWrite)
        .unwrap();

    sf.seek(SeekFrom::Start(3)).unwrap();
    let mut buf = [0u8; 3];
    sf.read(&mut buf).unwrap();
    assert_eq!(&buf, b"DEF");
    assert_eq!(sf.position(), 6);
}

#[test]
#[cfg(unix)]
fn test_symlink_within_root() {
    let tmp = TempDir::new("symlink_ok");
    fs::write(tmp.path().join("real.txt"), b"target file").unwrap();
    std::os::unix::fs::symlink("real.txt", tmp.path().join("link.txt")).unwrap();

    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    let data = native_fs.read_file("/link.txt").unwrap();
    assert_eq!(data, b"target file");
}

#[test]
fn test_capabilities() {
    let tmp = TempDir::new("caps");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    let caps = native_fs.capabilities();
    assert!(caps.persistent);
    assert!(caps.seekable);
}

#[test]
fn test_root_directory_exists() {
    let tmp = TempDir::new("root");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    assert!(native_fs.exists("/").unwrap());

    let dir = native_fs.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    assert!(entries.is_empty());
}

#[test]
fn test_create_file_and_read_with_handle() {
    let tmp = TempDir::new("create_handle");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());

    let file = native_fs.create("/test.txt", 0o644).unwrap();
    file.write_at(b"handle write", 0).unwrap();

    let data = native_fs.read_file("/test.txt").unwrap();
    assert_eq!(data, b"handle write");
}

#[test]
fn test_file_read_at_offset() {
    let tmp = TempDir::new("read_at");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    native_fs.write_file("/offset.bin", b"0123456789").unwrap();

    let file = native_fs.open("/offset.bin", OpenMode::Read).unwrap();
    let mut buf = [0u8; 3];
    let n = file.read_at(&mut buf, 5).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"567");
}

#[test]
fn test_remove_all() {
    let tmp = TempDir::new("remove_all");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    native_fs.mkdir("/tree").unwrap();
    native_fs.mkdir("/tree/sub").unwrap();
    native_fs.write_file("/tree/sub/f.txt", b"data").unwrap();
    native_fs.write_file("/tree/g.txt", b"data2").unwrap();

    native_fs.remove_all("/tree").unwrap();
    assert!(!native_fs.exists("/tree").unwrap());
}

#[test]
fn test_mkdir_all() {
    let tmp = TempDir::new("mkdir_all");
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).unwrap());
    native_fs.mkdir_all("/a/b/c").unwrap();
    assert!(native_fs.exists("/a/b/c").unwrap());
}

/// Bug fix: a path with `..` was mapped to the root directory, so
/// `remove("/a/..")` aimed at the root. It is now refused.
#[valtron_test]
async fn dot_dot_paths_are_refused_not_mapped_to_root() {
    let tmp = TempDir::new("dotdot");
    fs::write(tmp.path().join("keep.txt"), b"k").unwrap();
    let native_fs = NativeFs::new(tmp.path()).unwrap();

    assert!(native_fs.stat_async("/a/..".into()).await.is_err());
    assert!(native_fs.open_directory_async("..".into()).await.is_err());
    assert!(native_fs.remove_async("/keep.txt/..".into()).await.is_err());
    assert!(!native_fs.exists_async("/..".into()).await.unwrap());
    assert!(tmp.path().join("keep.txt").exists());
}

/// Bug fix: a directory handle joined names without the root checks, so
/// `dir.open_directory("..")` left the root.
#[valtron_test]
async fn directory_handles_cannot_leave_the_root() {
    let tmp = TempDir::new("dir_escape");
    fs::create_dir_all(tmp.path().join("sub")).unwrap();
    let native_fs = NativeFs::new(tmp.path()).unwrap();
    let sub = native_fs.open_directory_async("/sub".into()).await.unwrap();

    assert!(sub.open_directory_async("..".into()).await.is_err());
    assert!(sub.open_directory_async("../..".into()).await.is_err());
    assert!(sub.stat_async("../../etc/passwd".into()).await.is_err());
}

#[valtron_test]
async fn async_read_write_round_trip() {
    let tmp = TempDir::new("async_rw");
    let native_fs = NativeFs::new(tmp.path()).unwrap();
    native_fs
        .write_file_async("/a.txt".into(), b"hello".to_vec())
        .await
        .unwrap();
    assert_eq!(native_fs.read_file_async("/a.txt".into()).await.unwrap(), b"hello");
    let meta = native_fs.stat_async("/a.txt".into()).await.unwrap();
    assert_eq!(meta.size, 5);
}

/// Bug fix: `create` returned a `ReadWrite` handle over a write-only host
/// file, so a read on it failed with "bad file descriptor".
#[valtron_test]
async fn created_files_can_be_read_back() {
    let tmp = TempDir::new("create_rw");
    let native_fs = NativeFs::new(tmp.path()).unwrap();
    let f = native_fs.create_async("/n.txt".into(), 0o644).await.unwrap();
    f.write_at_async(b"abc".to_vec(), 0).await.unwrap();
    assert_eq!(f.read_at_async(3, 0).await.unwrap(), b"abc");
}
