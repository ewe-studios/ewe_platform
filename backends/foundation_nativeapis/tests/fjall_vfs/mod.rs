#![cfg(feature = "vfs-fjall")]

use foundation_nativeapis::native::vfs::fjall_fs::{FjallFs, FjallDelta, FjallVfsConfig};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, VfsFile, VfsFileSystem, VfsDirectory,
};

#[test]
fn test_fjall_fs_open_and_stat() {
    let tmp = std::env::temp_dir().join(format!("fjall-test-{}", std::process::id()));
    let fs = FjallFs::open(&tmp).unwrap();
    let meta = fs.stat("/").unwrap();
    assert_eq!(meta.size, 0);
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_fjall_fs_create_and_read() {
    let tmp = std::env::temp_dir().join(format!("fjall-test-{}", std::process::id()));
    let fs = FjallFs::open(&tmp).unwrap();
    let file = fs.create("/hello.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();
    let data = fs.read_file("/hello.txt").unwrap();
    assert_eq!(&data, b"hello world");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_fjall_fs_mkdir_and_list() {
    let tmp = std::env::temp_dir().join(format!("fjall-test-{}", std::process::id()));
    let fs = FjallFs::open(&tmp).unwrap();
    fs.mkdir("/src").unwrap();
    fs.create("/src/main.rs", 0o644).unwrap();
    let dir = fs.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    assert!(entries.iter().any(|e| e.name == "src"));
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_fjall_fs_rename() {
    let tmp = std::env::temp_dir().join(format!("fjall-test-{}", std::process::id()));
    let fs = FjallFs::open(&tmp).unwrap();
    fs.create("/old.txt", 0o644).unwrap();
    fs.rename("/old.txt", "/new.txt").unwrap();
    assert!(fs.exists("/new.txt").unwrap());
    assert!(!fs.exists("/old.txt").unwrap());
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_fjall_fs_remove() {
    let tmp = std::env::temp_dir().join(format!("fjall-test-{}", std::process::id()));
    let fs = FjallFs::open(&tmp).unwrap();
    fs.create("/toremove.txt", 0o644).unwrap();
    assert!(fs.exists("/toremove.txt").unwrap());
    fs.remove("/toremove.txt").unwrap();
    assert!(!fs.exists("/toremove.txt").unwrap());
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_fjall_fs_in_memory() {
    let fs = FjallFs::in_memory().unwrap();
    fs.create("/mem.txt", 0o644).unwrap();
    let data = fs.read_file("/mem.txt").unwrap();
    assert_eq!(data, b"");
}

#[test]
fn test_fjall_delta_whiteout() {
    let tmp = std::env::temp_dir().join(format!("fjall-delta-test-{}", std::process::id()));
    let delta = FjallDelta::open(&tmp).unwrap();
    delta.add_whiteout("/hidden.txt", 1).unwrap();
    assert!(delta.is_whiteout("/hidden.txt").unwrap().is_some());
    delta.remove_whiteout("/hidden.txt").unwrap();
    assert!(delta.is_whiteout("/hidden.txt").unwrap().is_none());
    let _ = std::fs::remove_dir_all(&tmp);
}
