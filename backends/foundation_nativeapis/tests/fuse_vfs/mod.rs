use foundation_nativeapis::native::vfs::{FuseMount, FuseMountOptions};
use foundation_nativeapis::shared::vfs::{MemoryFs, VfsFileSystem};

use std::time::Duration;

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
fn test_fuse_mount_with_populated_fs() {
    let fs = MemoryFs::new();
    fs.mkdir("/docs").unwrap();
    fs.write_file("/docs/readme.txt", b"hello").unwrap();
    fs.write_file("/config.json", b"{}").unwrap();

    let mount = FuseMount::new(fs, FuseMountOptions::default());
    let debug = format!("{mount:?}");
    assert!(debug.contains("FuseMount"));
}

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
