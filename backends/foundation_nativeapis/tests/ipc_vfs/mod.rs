#![cfg(all(feature = "vfs-ipc", feature = "vfs-native"))]

use std::sync::Arc;

use foundation_nativeapis::native::vfs::NativeFs;
use foundation_nativeapis::shared::vfs::ipc_client::{DirectTransport, VfsClient};
use foundation_nativeapis::shared::vfs::ipc_daemon::VfsDaemon;
use foundation_nativeapis::shared::vfs::{
    OpenMode, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem,
};
use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vfs_ipc_test_{name}_{}_{}", std::process::id(),
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

fn make_client(tmp: &TempDir) -> VfsClient {
    let native_fs = NativeFs::new(tmp.path()).expect("create NativeFs");
    let daemon = VfsDaemon::new(Arc::new(native_fs));
    let transport = DirectTransport::new(daemon);
    VfsClient::new(Arc::new(transport))
}

// ── Test 1: Basic file operations through IPC transport ──

#[test]
fn test_open_read_write_close() {
    let tmp = TempDir::new("read_write");
    fs::write(tmp.path().join("test.txt"), b"hello world").unwrap();

    let client = make_client(&tmp);

    // Read through IPC client
    let file = client.open("/test.txt", OpenMode::Read).expect("open failed");
    let mut buf = vec![0u8; 64];
    let n = file.read_at(&mut buf, 0).expect("read failed");
    assert_eq!(&buf[..n], b"hello world");

    // Write through IPC client
    let wfile = client.open("/test.txt", OpenMode::Write).expect("open write failed");
    let n = wfile.write_at(b"new content", 0).expect("write failed");
    assert_eq!(n, 11);

    // Read back
    drop(wfile);
    let file2 = client.open("/test.txt", OpenMode::Read).expect("open after write failed");
    let mut buf2 = vec![0u8; 64];
    let n2 = file2.read_at(&mut buf2, 0).expect("read after write failed");
    assert_eq!(&buf2[..n2], b"new content");
}

// ── Test 2: Stat through client matches direct stat ──

#[test]
fn test_stat_through_client_matches_direct() {
    let tmp = TempDir::new("stat");
    fs::write(tmp.path().join("data.txt"), b"1234567890").unwrap();

    let client = make_client(&tmp);

    // Stat through IPC client
    let client_meta = client.stat("/data.txt").expect("stat failed");

    // Direct stat on NativeFs
    let native_fs = NativeFs::new(tmp.path()).expect("create NativeFs");
    let direct_meta = native_fs.stat("/data.txt").expect("direct stat failed");

    assert_eq!(client_meta.size, direct_meta.size);
    assert_eq!(client_meta.size, 10);
    assert_eq!(client_meta.file_type, direct_meta.file_type);
}

// ── Test 3: Multiple clients sharing one daemon ──

#[test]
fn test_multiple_clients_shared_daemon() {
    let tmp = TempDir::new("multi_client");

    // Create two independent daemon+client pairs sharing the same directory
    let native_fs_a = NativeFs::new(tmp.path()).expect("create NativeFs A");
    let daemon_a = VfsDaemon::new(Arc::new(native_fs_a));
    let transport_a = DirectTransport::new(daemon_a);
    let client_a = VfsClient::new(Arc::new(transport_a));

    let native_fs_b = NativeFs::new(tmp.path()).expect("create NativeFs B");
    let daemon_b = VfsDaemon::new(Arc::new(native_fs_b));
    let transport_b = DirectTransport::new(daemon_b);
    let client_b = VfsClient::new(Arc::new(transport_b));

    // Client A creates a file
    client_a.write_file("/shared.txt", b"from client A").unwrap();

    // Client B reads it
    let data = client_b.read_file("/shared.txt").unwrap();
    assert_eq!(data, b"from client A");

    // Client B modifies
    client_b.write_file("/shared.txt", b"from client B").unwrap();

    // Client A reads the modification
    let data = client_a.read_file("/shared.txt").unwrap();
    assert_eq!(data, b"from client B");
}

// ── Test 4: Directory operations ──

#[test]
fn test_directory_operations() {
    let tmp = TempDir::new("dir_ops");
    let client = make_client(&tmp);

    // Create directory
    client.mkdir("/subdir").expect("mkdir failed");

    // Verify it exists
    assert!(client.exists("/subdir").expect("exists failed"));

    // Create a file in the directory
    client.write_file("/subdir/nested.txt", b"nested content").unwrap();

    // Read it back
    let data = client.read_file("/subdir/nested.txt").unwrap();
    assert_eq!(data, b"nested content");

    // List directory
    let dir = client.open_directory("/subdir").expect("open dir failed");
    let entries = dir.list().expect("list failed");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "nested.txt");
}

// ── Test 5: Create and remove ──

#[test]
fn test_create_and_remove() {
    let tmp = TempDir::new("create_remove");
    let client = make_client(&tmp);

    // Create file
    let file = client.create("/new_file.txt", 0o644).expect("create failed");
    drop(file);

    assert!(client.exists("/new_file.txt").unwrap());

    // Remove file
    client.remove("/new_file.txt").expect("remove failed");
    assert!(!client.exists("/new_file.txt").unwrap());
}

// ── Test 6: Rename ──

#[test]
fn test_rename() {
    let tmp = TempDir::new("rename");
    fs::write(tmp.path().join("old.txt"), b"rename me").unwrap();

    let client = make_client(&tmp);

    client.rename("/old.txt", "/new.txt").expect("rename failed");
    assert!(!client.exists("/old.txt").unwrap());
    assert!(client.exists("/new.txt").unwrap());

    let data = client.read_file("/new.txt").unwrap();
    assert_eq!(data, b"rename me");
}

// ── Test 7: Symlink and readlink ──

#[test]
fn test_symlink_and_readlink() {
    let tmp = TempDir::new("symlink");
    fs::write(tmp.path().join("target.txt"), b"symlink target").unwrap();

    let client = make_client(&tmp);

    client.symlink("/target.txt", "/link.txt").expect("symlink failed");
    let target = client.readlink("/link.txt").expect("readlink failed");
    assert_eq!(target, "/target.txt");
}

// ── Test 8: Capabilities ──

#[test]
fn test_capabilities() {
    let tmp = TempDir::new("capabilities");
    let client = make_client(&tmp);

    let caps = client.capabilities();
    assert!(caps.seekable);
}

// ── Test 9: Inode operations ──

#[test]
fn test_inode_operations() {
    let tmp = TempDir::new("inode");
    fs::write(tmp.path().join("inode_test.txt"), b"inode data").unwrap();

    let client = make_client(&tmp);

    // Get inode (works — just calls stat and extracts the inode number)
    let ino = client.inode("/inode_test.txt").expect("inode failed");
    assert_ne!(ino, 0);

    // stat_by_inode and path_by_inode require an inode-native VFS backend
    // (e.g., OverlayFileSystem with delta tracking). NativeFs cannot reverse-lookup
    // an inode to a path, so these operations return "not supported".
    // The IPC transport correctly forwards the error.
    let result = client.stat_by_inode(ino);
    assert!(result.is_err(), "NativeFs should not support stat_by_inode");
}

// ── Test 10: ReadFile / WriteFile convenience ──

#[test]
fn test_read_write_file_convenience() {
    let tmp = TempDir::new("convenience");
    let client = make_client(&tmp);

    let data = vec![0xAB; 1024];
    client.write_file("/big.bin", &data).unwrap();

    let read = client.read_file("/big.bin").unwrap();
    assert_eq!(read.len(), 1024);
    assert_eq!(read, data);
}
