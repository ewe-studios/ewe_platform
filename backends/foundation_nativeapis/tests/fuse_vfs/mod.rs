use foundation_nativeapis::native::vfs::fuse::{
    flags_to_open_mode, vfs_error_to_errno, FuseMount, FuseMountOptions, ROOT_INO,
};
use foundation_nativeapis::shared::vfs::{MemoryFs, OpenMode, VfsDirectory, VfsError, VfsFileSystem, VfsFileType, VfsMetadata};

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

// ── Construction ──

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

    let meta = VfsMetadata::new_file(5, 1024, 0o644);
    let attr = mount.metadata_to_attr(&meta);

    assert_eq!(attr.ino, 5);
    assert_eq!(attr.size, 1024);
    assert_eq!(attr.perm, 0o644);
    assert_eq!(attr.nlink, 1);
}

#[test]
fn test_metadata_to_attr_directory() {
    let fs = MemoryFs::new();
    let mount = FuseMount::new(fs, FuseMountOptions::default());

    let meta = VfsMetadata::new_directory(3, 0o755);
    let attr = mount.metadata_to_attr(&meta);

    assert_eq!(attr.ino, 3);
    assert_eq!(attr.perm, 0o755);
    assert_eq!(attr.nlink, 2);
}

// ── VFS-Native Inode Tests ──

#[test]
fn test_memory_fs_root_has_inode_1() {
    let fs = MemoryFs::new();
    let ino = fs.inode("/").unwrap();
    assert_eq!(ino, ROOT_INO);
}

#[test]
fn test_memory_fs_inode_monotonic_allocation() {
    let fs = MemoryFs::new();
    fs.mkdir("/a").unwrap();
    fs.mkdir("/b").unwrap();
    fs.write_file("/c.txt", b"data").unwrap();

    let ino_a = fs.inode("/a").unwrap();
    let ino_b = fs.inode("/b").unwrap();
    let ino_c = fs.inode("/c.txt").unwrap();

    assert!(ino_a >= 2);
    assert!(ino_b > ino_a);
    assert!(ino_c > ino_b);
}

#[test]
fn test_memory_fs_path_by_inode_roundtrip() {
    let fs = MemoryFs::new();
    fs.mkdir("/docs").unwrap();
    fs.write_file("/docs/readme.txt", b"hello").unwrap();

    let ino = fs.inode("/docs/readme.txt").unwrap();
    let path = fs.path_by_inode(ino).unwrap();
    assert_eq!(path, "/docs/readme.txt");
}

#[test]
fn test_memory_fs_stat_by_inode() {
    let fs = MemoryFs::new();
    fs.write_file("/test.bin", b"content").unwrap();

    let ino = fs.inode("/test.bin").unwrap();
    let meta = fs.stat_by_inode(ino).unwrap();

    assert_eq!(meta.inode, ino);
    assert_eq!(meta.file_type, VfsFileType::Regular);
    assert_eq!(meta.size, 7);
}

#[test]
fn test_memory_fs_rename_preserves_inode() {
    let fs = MemoryFs::new();
    fs.write_file("/old.txt", b"data").unwrap();

    let ino_before = fs.inode("/old.txt").unwrap();
    fs.rename("/old.txt", "/new.txt").unwrap();

    let ino_after = fs.inode("/new.txt").unwrap();
    assert_eq!(ino_before, ino_after);

    let path = fs.path_by_inode(ino_after).unwrap();
    assert_eq!(path, "/new.txt");
}

#[test]
fn test_memory_fs_remove_invalidates_inode() {
    let fs = MemoryFs::new();
    fs.write_file("/gone.txt", b"bye").unwrap();

    let ino = fs.inode("/gone.txt").unwrap();
    fs.remove("/gone.txt").unwrap();

    assert!(fs.path_by_inode(ino).is_err());
}

#[test]
fn test_memory_fs_stat_populates_inode() {
    let fs = MemoryFs::new();
    fs.mkdir("/subdir").unwrap();

    let meta = fs.stat("/subdir").unwrap();
    assert!(meta.inode >= 2);
    assert_eq!(meta.file_type, VfsFileType::Directory);
}

#[test]
fn test_memory_fs_dir_entries_have_inodes() {
    let fs = MemoryFs::new();
    fs.mkdir("/dir").unwrap();
    fs.write_file("/dir/a.txt", b"a").unwrap();
    fs.write_file("/dir/b.txt", b"b").unwrap();

    let dir = fs.open_directory("/dir").unwrap();
    let entries = dir.list().unwrap();

    for entry in &entries {
        assert!(entry.inode >= 2, "entry {} has inode 0", entry.name);
        let resolved = fs.path_by_inode(entry.inode).unwrap();
        assert!(resolved.ends_with(&entry.name));
    }
}

// ── Populated FS ──

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
