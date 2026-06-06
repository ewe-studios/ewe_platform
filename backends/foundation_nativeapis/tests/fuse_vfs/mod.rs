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
