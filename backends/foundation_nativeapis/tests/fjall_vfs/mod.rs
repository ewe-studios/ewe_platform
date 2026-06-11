#![cfg(feature = "vfs-fjall")]

use foundation_nativeapis::shared::vfs::fjall_fs::{FjallFs, FjallDelta, FjallVfsConfig};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, SeekFrom, SeekableVfsFile, VfsDirectory, VfsFile,
    VfsFileSystem, VfsFileType,
};
use tracing_test::traced_test;

// ── FjallFs: File Operations ──

#[test]
#[traced_test]
fn test_fjall_create_and_read_file() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/hello.txt", 0o644).unwrap();
    file.write_at(b"hello world", 0).unwrap();
    let mut buf = vec![0u8; 11];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(n, 11);
    assert_eq!(&buf, b"hello world");
}

#[test]
#[traced_test]
fn test_fjall_create_file_in_nonexistent_dir() {
    let fs = FjallFs::in_memory().unwrap();
    let result = fs.create("/no/such/dir/file.txt", 0o644);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_fjall_open_nonexistent_file() {
    let fs = FjallFs::in_memory().unwrap();
    let result = fs.open("/nope.txt", OpenMode::Read);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_fjall_write_to_read_only_file() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/readonly.txt", 0o644).unwrap();
    file.write_at(b"initial", 0).unwrap();
    let ro = fs.open("/readonly.txt", OpenMode::Read).unwrap();
    let result = ro.write_at(b"nope", 0);
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_fjall_file_read_at_offset() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/data.bin", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();
    let mut buf = [0u8; 3];
    let n = file.read_at(&mut buf, 5).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"FGH");
}

#[test]
#[traced_test]
fn test_fjall_file_write_at_offset() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/sparse.bin", 0o644).unwrap();
    file.write_at(b"XYZ", 10).unwrap();
    assert_eq!(file.size().unwrap(), 13);
    let mut buf = [0u8; 13];
    file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf[..10], &[0u8; 10]);
    assert_eq!(&buf[10..], b"XYZ");
}

#[test]
#[traced_test]
fn test_fjall_file_truncate() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/trunc.txt", 0o644).unwrap();
    file.write_at(b"123456789", 0).unwrap();
    file.truncate(3).unwrap();
    assert_eq!(file.size().unwrap(), 3);
    let mut buf = [0u8; 3];
    file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf, b"123");
}

#[test]
#[traced_test]
fn test_fjall_seekable_sequential_read() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/seq.txt", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();
    let mut sf = fs.open_seekable("/seq.txt", OpenMode::Read).unwrap();
    let mut buf = vec![0u8; 5];
    let n = sf.read(&mut buf).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf[..n], b"ABCDE");
}

#[test]
#[traced_test]
fn test_fjall_seekable_seek_and_read() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/seek.txt", 0o644).unwrap();
    file.write_at(b"ABCDEFGHIJ", 0).unwrap();
    let mut sf = fs.open_seekable("/seek.txt", OpenMode::Read).unwrap();
    sf.seek(std::io::SeekFrom::Start(5)).unwrap();
    let mut buf = vec![0u8; 3];
    sf.read(&mut buf).unwrap();
    assert_eq!(&buf, b"FGH");
}

// ── FjallFs: Directory Operations ──

#[test]
#[traced_test]
fn test_fjall_mkdir_and_list() {
    let fs = FjallFs::in_memory().unwrap();
    fs.mkdir("/src").unwrap();
    fs.create("/src/main.rs", 0o644).unwrap();
    let dir = fs.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    assert!(entries.iter().any(|e| e.name == "src"));
}

#[test]
#[traced_test]
fn test_fjall_mkdir_nested() {
    let fs = FjallFs::in_memory().unwrap();
    fs.mkdir("/a/b/c").unwrap();
    assert!(fs.exists("/a/b/c").unwrap());
}

#[test]
#[traced_test]
fn test_fjall_remove_empty_dir() {
    let fs = FjallFs::in_memory().unwrap();
    fs.mkdir("/empty").unwrap();
    fs.remove("/empty").unwrap();
    assert!(!fs.exists("/empty").unwrap());
}

#[test]
#[traced_test]
fn test_fjall_readdir_mixed() {
    let fs = FjallFs::in_memory().unwrap();
    fs.mkdir("/dir").unwrap();
    fs.create("/dir/file.txt", 0o644).unwrap();
    fs.mkdir("/dir/sub").unwrap();
    let dir = fs.open_directory("/dir").unwrap();
    let entries = dir.list().unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
#[traced_test]
fn test_fjall_stat_returns_correct_metadata() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/stat.txt", 0o644).unwrap();
    file.write_at(b"hello", 0).unwrap();
    let meta = fs.stat("/stat.txt").unwrap();
    assert_eq!(meta.size, 5);
    assert_eq!(meta.file_type, VfsFileType::Regular);
}

#[test]
#[traced_test]
fn test_fjall_chmod_updates_permissions() {
    let fs = FjallFs::in_memory().unwrap();
    fs.create("/perm.txt", 0o644).unwrap();
    fs.chmod("/perm.txt", 0o755).unwrap();
    let meta = fs.stat("/perm.txt").unwrap();
    assert_eq!(meta.permissions, 0o755);
}

#[test]
#[traced_test]
fn test_fjall_symlink_create_and_readlink() {
    let fs = FjallFs::in_memory().unwrap();
    fs.symlink("/target", "/link").unwrap();
    let target = fs.readlink("/link").unwrap();
    assert_eq!(target, "/target");
}

// ── FjallFs: Convenience Operations ──

#[test]
#[traced_test]
fn test_fjall_read_file_convenience() {
    let fs = FjallFs::in_memory().unwrap();
    let file = fs.create("/rf.txt", 0o644).unwrap();
    file.write_at(b"data", 0).unwrap();
    let data = fs.read_file("/rf.txt").unwrap();
    assert_eq!(&data, b"data");
}

#[test]
#[traced_test]
fn test_fjall_write_file_convenience() {
    let fs = FjallFs::in_memory().unwrap();
    fs.create("/wf.txt", 0o644).unwrap();
    fs.write_file("/wf.txt", b"written").unwrap();
    let data = fs.read_file("/wf.txt").unwrap();
    assert_eq!(&data, b"written");
}

#[test]
#[traced_test]
fn test_fjall_mkdir_all() {
    let fs = FjallFs::in_memory().unwrap();
    fs.mkdir_all("/a/b/c/d").unwrap();
    assert!(fs.exists("/a/b/c/d").unwrap());
}

#[test]
#[traced_test]
fn test_fjall_remove_all() {
    let fs = FjallFs::in_memory().unwrap();
    fs.mkdir_all("/parent/child").unwrap();
    fs.create("/parent/child/file.txt", 0o644).unwrap();
    fs.remove_all("/parent").unwrap();
    assert!(!fs.exists("/parent").unwrap());
}

// ── FjallDelta: Whiteout Operations ──

#[test]
#[traced_test]
fn test_fjall_whiteout_add_and_check() {
    let delta = FjallDelta::in_memory().unwrap();
    delta.add_whiteout("/hidden.txt", 1).unwrap();
    assert!(delta.is_whiteout("/hidden.txt").unwrap().is_some());
}

#[test]
#[traced_test]
fn test_fjall_whiteout_remove() {
    let delta = FjallDelta::in_memory().unwrap();
    delta.add_whiteout("/gone.txt", 1).unwrap();
    delta.remove_whiteout("/gone.txt").unwrap();
    assert!(delta.is_whiteout("/gone.txt").unwrap().is_none());
}

#[test]
#[traced_test]
fn test_fjall_whiteout_list() {
    let delta = FjallDelta::in_memory().unwrap();
    delta.add_whiteout("/dir/a.txt", 1).unwrap();
    delta.add_whiteout("/dir/b.txt", 2).unwrap();
    delta.add_whiteout("/other.txt", 3).unwrap();
    let whiteouts = delta.list_whiteouts("/dir").unwrap();
    assert_eq!(whiteouts.len(), 2);
}

#[test]
#[traced_test]
fn test_fjall_whiteout_reset_clears_all() {
    let delta = FjallDelta::in_memory().unwrap();
    delta.add_whiteout("/a.txt", 1).unwrap();
    delta.add_whiteout("/b.txt", 2).unwrap();
    delta.reset().unwrap();
    assert!(delta.list_whiteouts("/").unwrap().is_empty());
}

#[test]
#[traced_test]
fn test_fjall_delta_create_and_read() {
    let delta = FjallDelta::in_memory().unwrap();
    delta.create("/delta.txt", 0o644).unwrap();
    let file = delta.open("/delta.txt", OpenMode::Write).unwrap();
    file.write_at(b"delta content", 0).unwrap();
    let data = delta.read_file("/delta.txt").unwrap();
    assert_eq!(&data, b"delta content");
}

#[test]
#[traced_test]
fn test_fjall_delta_exists_respects_whiteout() {
    let delta = FjallDelta::in_memory().unwrap();
    delta.create("/visible.txt", 0o644).unwrap();
    assert!(delta.exists("/visible.txt").unwrap());
    delta.add_whiteout("/visible.txt", 1).unwrap();
    assert!(!delta.exists("/visible.txt").unwrap());
}

// ── FjallFs: Inode Operations ──

#[test]
#[traced_test]
fn test_fjall_inode() {
    let fs = FjallFs::in_memory().unwrap();
    fs.create("/inode.txt", 0o644).unwrap();
    fs.write_file("/inode.txt", b"data").unwrap();
    let ino = fs.inode("/inode.txt").unwrap();
    assert!(ino > 0);
}

#[test]
#[traced_test]
fn test_fjall_path_by_inode() {
    let fs = FjallFs::in_memory().unwrap();
    fs.create("/path_lookup.txt", 0o644).unwrap();
    fs.write_file("/path_lookup.txt", b"data").unwrap();
    let ino = fs.inode("/path_lookup.txt").unwrap();
    let path = fs.path_by_inode(ino).unwrap();
    assert_eq!(path, "/path_lookup.txt");
}

#[test]
#[traced_test]
fn test_fjall_stat_by_inode() {
    let fs = FjallFs::in_memory().unwrap();
    fs.create("/stat_ino.txt", 0o644).unwrap();
    fs.write_file("/stat_ino.txt", b"data").unwrap();
    let ino = fs.inode("/stat_ino.txt").unwrap();
    let meta = fs.stat_by_inode(ino).unwrap();
    assert_eq!(meta.size, 4);
}
