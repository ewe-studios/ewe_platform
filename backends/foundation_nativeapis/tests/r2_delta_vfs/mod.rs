#![cfg(feature = "vfs-r2")]

use foundation_nativeapis::native::vfs::r2_delta::{R2Delta, R2File, R2SeekableFile, R2Directory};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem,
};

/// Verify R2Delta implements all required traits (compile-time check).
#[test]
fn test_r2_delta_trait_impls() {
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    fn assert_delta_store<T: DeltaStore>() {}
    assert_vfs_fs::<R2Delta>();
    assert_delta_store::<R2Delta>();
}

/// Verify R2Delta associated types are correct.
#[test]
fn test_r2_delta_associated_types() {
    fn assert_file_type<T: VfsFileSystem<File = R2File, SeekableFile = R2SeekableFile, Directory = R2Directory>>() {}
    assert_file_type::<R2Delta>();
}

/// Verify R2File implements VfsFile.
#[test]
fn test_r2_file_trait_impls() {
    fn assert_vfs_file<T: VfsFile>() {}
    fn assert_seekable<T: SeekableVfsFile>() {}
    assert_vfs_file::<R2File>();
    assert_vfs_file::<R2SeekableFile>();
    assert_seekable::<R2SeekableFile>();
}

/// Verify R2Directory implements VfsDirectory.
#[test]
fn test_r2_directory_trait_impls() {
    fn assert_vfs_dir<T: VfsDirectory>() {}
    assert_vfs_dir::<R2Directory>();
}

/// Verify R2Delta can be constructed with test credentials.
#[test]
fn test_r2_delta_new() {
    let delta = R2Delta::new("test-token", "test-account", "test-bucket");
    assert!(delta.is_ok());
}

/// Verify from_env fails gracefully when env vars are missing.
#[test]
fn test_r2_delta_from_env_no_creds() {
    let result = R2Delta::from_env();
    assert!(result.is_err());
}

/// Verify R2Delta capabilities.
#[test]
fn test_r2_delta_capabilities() {
    let delta = R2Delta::new("test", "test", "test").unwrap();
    let caps = delta.capabilities();
    assert!(caps.seekable);
    assert!(caps.persistent);
    assert!(!caps.symlinks);
    assert!(!caps.permissions_enforced);
    assert!(!caps.event_emission);
}
