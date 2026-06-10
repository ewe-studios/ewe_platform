#![cfg(feature = "vfs-d1")]

use foundation_nativeapis::native::vfs::d1_delta::{D1Delta, D1File, D1SeekableFile, D1Directory};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem,
};

/// Verify D1Delta implements all required traits (compile-time check).
#[test]
fn test_d1_delta_trait_impls() {
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    fn assert_delta_store<T: DeltaStore>() {}
    assert_vfs_fs::<D1Delta>();
    assert_delta_store::<D1Delta>();
}

/// Verify D1Delta associated types are correct.
#[test]
fn test_d1_delta_associated_types() {
    fn assert_file_type<T: VfsFileSystem<File = D1File, SeekableFile = D1SeekableFile, Directory = D1Directory>>() {}
    assert_file_type::<D1Delta>();
}

/// Verify D1File implements VfsFile.
#[test]
fn test_d1_file_trait_impls() {
    fn assert_vfs_file<T: VfsFile>() {}
    fn assert_seekable<T: SeekableVfsFile>() {}
    assert_vfs_file::<D1File>();
    assert_vfs_file::<D1SeekableFile>();
    assert_seekable::<D1SeekableFile>();
}

/// Verify D1Directory implements VfsDirectory.
#[test]
fn test_d1_directory_trait_impls() {
    fn assert_vfs_dir<T: VfsDirectory>() {}
    assert_vfs_dir::<D1Directory>();
}

/// Verify D1Delta can be constructed with test credentials.
/// This tests the constructor path, not actual HTTP calls.
#[test]
fn test_d1_delta_new() {
    let delta = D1Delta::new("test-token", "test-account", "test-db");
    assert!(delta.is_ok());
}

/// Verify from_env fails gracefully when env vars are missing.
#[test]
fn test_d1_delta_from_env_no_creds() {
    // Without env vars set, from_env should fail
    let result = D1Delta::from_env();
    // Should fail because CF_ACCOUNT_ID etc are not set
    assert!(result.is_err());
}

/// Verify D1Delta capabilities.
#[test]
fn test_d1_delta_capabilities() {
    let delta = D1Delta::new("test", "test", "test").unwrap();
    let caps = delta.capabilities();
    assert!(caps.seekable);
    assert!(caps.persistent);
    assert!(!caps.symlinks);
    assert!(!caps.permissions_enforced);
    assert!(!caps.event_emission);
}
