use foundation_nativeapis::native::vfs::fuse::{
    flags_to_open_mode, vfs_error_to_errno, FuseMount, FuseMountOptions, ROOT_INO,
};
use foundation_nativeapis::shared::vfs::{MemoryFs, OpenMode, VfsError, VfsFileSystem, VfsFileType, VfsMetadata};

use std::time::Duration;

// ── Error Mapping ──

#[test]
fn test_vfs_error_to_errno_mapping() {
    assert_eq!(vfs_error_to_errno(&VfsError::NotFound { path: "/x".into() }), libc::ENOENT);
    assert_eq!(vfs_error_to_errno(&VfsError::AlreadyExists { path: "/x".into() }), libc::EEXIST);
    assert_eq!(vfs_error_to_errno(&VfsError::PermissionDenied { path: "/x".into() }), libc::EACCES);
    assert_eq!(vfs_error_to_errno(&VfsError::NotAFile { path: "/x".into() }), libc::EISDIR);
    assert_eq!(vfs_error_to_errno(&VfsError::NotADirectory { path: "/x".into() }), libc::ENOTDIR);
    assert_eq!(vfs_error_to_errno(&VfsError::Unsupported { operation: "op".into() }), libc::ENOSYS);
    assert_eq!(vfs_error_to_errno(&VfsError::InvalidPath { path: "/x".into() }), libc::EINVAL);
    assert_eq!(vfs_error_to_errno(&VfsError::ReadOnly), libc::EROFS);
    assert_eq!(vfs_error_to_errno(&VfsError::EntryPending { path: "/x".into() }), libc::EAGAIN);
    assert_eq!(vfs_error_to_errno(&VfsError::SymlinkLoop { path: "/x".into() }), libc::ELOOP);
    assert_eq!(vfs_error_to_errno(&VfsError::DirectoryNotEmpty { path: "/x".into() }), libc::ENOTEMPTY);
    assert_eq!(vfs_error_to_errno(&VfsError::Backend { message: "err".into() }), libc::EIO);
}

// ── Flags / Options ──

#[test]
fn test_flags_to_open_mode() {
    assert_eq!(flags_to_open_mode(libc::O_RDONLY), OpenMode::Read);
    assert_eq!(flags_to_open_mode(libc::O_WRONLY), OpenMode::Write);
    assert_eq!(flags_to_open_mode(libc::O_RDWR), OpenMode::ReadWrite);
}

#[test]
fn test_default_options() {
    let opts = FuseMountOptions::default();
    assert_eq!(opts.attr_timeout, Duration::from_secs(1));
    assert_eq!(opts.entry_timeout, Duration::from_secs(1));
    assert!(opts.auto_unmount);
    assert!(!opts.allow_other);
}

// ── Child Path ──

#[test]
fn test_child_path() {
    assert_eq!(FuseMount::<MemoryFs>::child_path("/", "foo"), "/foo");
    assert_eq!(FuseMount::<MemoryFs>::child_path("/bar", "baz"), "/bar/baz");
}

// ── Construction / Inode Cache ──

#[test]
fn test_fuse_mount_construction() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());
    let debug = format!("{mount:?}");
    assert!(debug.contains("FuseMount"));
}

#[test]
fn test_fuse_mount_custom_options() {
    let opts = FuseMountOptions {
        attr_timeout: Duration::from_secs(60),
        entry_timeout: Duration::from_secs(300),
        auto_unmount: false,
        allow_other: true,
    };
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, opts);
    let debug = format!("{mount:?}");
    assert!(debug.contains("FuseMount"));
}

#[test]
fn test_fuse_mount_new_initializes_root_inode() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let inodes = mount.inodes.read().unwrap();
    assert!(inodes.contains_key(&ROOT_INO));
    let root = &inodes[&ROOT_INO];
    assert_eq!(root.path, "/");
    assert_eq!(root.file_type, VfsFileType::Directory);
    assert_eq!(root.refcount, u64::MAX);

    let p2i = mount.path_to_ino.read().unwrap();
    assert_eq!(p2i.get("/"), Some(&ROOT_INO));
}

#[test]
fn test_inode_allocation_is_monotonic() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let first = mount.alloc_ino();
    let second = mount.alloc_ino();
    let third = mount.alloc_ino();

    assert_eq!(first, 2);
    assert_eq!(second, 3);
    assert_eq!(third, 4);
}

#[test]
fn test_lookup_or_insert_creates_and_increments() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let ino1 = mount.lookup_or_insert("/foo", VfsFileType::Regular);
    assert_eq!(ino1, 2);

    let ino2 = mount.lookup_or_insert("/foo", VfsFileType::Regular);
    assert_eq!(ino2, ino1);

    let inodes = mount.inodes.read().unwrap();
    assert_eq!(inodes[&ino1].refcount, 2);
}

#[test]
fn test_file_handle_allocation() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let fh1 = mount.alloc_fh();
    let fh2 = mount.alloc_fh();

    assert_eq!(fh1, 1);
    assert_eq!(fh2, 2);
}

// ── Metadata to FileAttr ──

#[test]
fn test_metadata_to_attr_regular_file() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let meta = VfsMetadata::new_file(1024, 0o644);
    let attr = mount.metadata_to_attr(5, &meta);

    assert_eq!(attr.ino, 5);
    assert_eq!(attr.size, 1024);
    assert_eq!(attr.perm, 0o644);
    assert_eq!(attr.nlink, 1);
}

#[test]
fn test_metadata_to_attr_directory() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let meta = VfsMetadata::new_directory(0o755);
    let attr = mount.metadata_to_attr(3, &meta);

    assert_eq!(attr.ino, 3);
    assert_eq!(attr.perm, 0o755);
    assert_eq!(attr.nlink, 2);
}

#[test]
fn test_fuse_mount_with_populated_fs() {
    let fs = MemoryFs::new();
    fs.mkdir("/docs").unwrap();
    fs.write_file("/docs/readme.txt", b"hello").unwrap();
    fs.write_file("/config.json", b"{}").unwrap();

    let mount = FuseMount::new(fs, FuseMountOptions::default());
    let debug = format!("{mount:?}");
    assert!(debug.contains("FuseMount"));
}

// ── FUSE Mount Integration (requires /dev/fuse) ──

fn fuse_available() -> bool {
    std::path::Path::new("/dev/fuse").exists()
}

fn create_temp_mountpoint() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create temp dir for FUSE mount")
}

#[test]
fn test_fuse_mount_and_read_file() {
    if !fuse_available() {
        eprintln!("skipping: /dev/fuse not available");
        return;
    }

    let fs = MemoryFs::new();
    fs.write_file("/hello.txt", b"hello from VFS").unwrap();
    fs.mkdir("/subdir").unwrap();
    fs.write_file("/subdir/nested.txt", b"nested content").unwrap();

    let mountpoint = create_temp_mountpoint();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let bg = match mount.mount_background(mountpoint.path().to_str().unwrap()) {
        Ok(bg) => bg,
        Err(e) => {
            eprintln!("skipping: FUSE mount failed (permissions?): {e}");
            return;
        }
    };

    std::thread::sleep(Duration::from_millis(200));

    let content = std::fs::read_to_string(mountpoint.path().join("hello.txt")).unwrap();
    assert_eq!(content, "hello from VFS");

    let nested = std::fs::read_to_string(mountpoint.path().join("subdir/nested.txt")).unwrap();
    assert_eq!(nested, "nested content");

    let entries: Vec<String> = std::fs::read_dir(mountpoint.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(entries.contains(&"hello.txt".to_string()));
    assert!(entries.contains(&"subdir".to_string()));

    drop(bg);
}

#[test]
fn test_fuse_write_through_mount() {
    if !fuse_available() {
        eprintln!("skipping: /dev/fuse not available");
        return;
    }

    let fs = MemoryFs::new();
    fs.write_file("/existing.txt", b"original").unwrap();

    let mountpoint = create_temp_mountpoint();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let bg = match mount.mount_background(mountpoint.path().to_str().unwrap()) {
        Ok(bg) => bg,
        Err(e) => {
            eprintln!("skipping: FUSE mount failed (permissions?): {e}");
            return;
        }
    };

    std::thread::sleep(Duration::from_millis(200));

    std::fs::write(mountpoint.path().join("new_file.txt"), b"new content").unwrap();

    let content = std::fs::read_to_string(mountpoint.path().join("new_file.txt")).unwrap();
    assert_eq!(content, "new content");

    drop(bg);
}

#[test]
fn test_fuse_unmount_cleanly() {
    if !fuse_available() {
        eprintln!("skipping: /dev/fuse not available");
        return;
    }

    let fs = MemoryFs::new();
    let mountpoint = create_temp_mountpoint();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let bg = match mount.mount_background(mountpoint.path().to_str().unwrap()) {
        Ok(bg) => bg,
        Err(e) => {
            eprintln!("skipping: FUSE mount failed (permissions?): {e}");
            return;
        }
    };

    std::thread::sleep(Duration::from_millis(200));

    assert!(mountpoint.path().exists());

    bg.join();

    assert!(mountpoint.path().exists());
}
