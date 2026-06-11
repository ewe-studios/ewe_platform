#![cfg(feature = "vfs-r2")]

use foundation_nativeapis::shared::vfs::r2_delta::R2Delta;
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, VfsFile, VfsFileSystem,
};

// Note: R2Delta requires Cloudflare credentials (CF_ACCOUNT_ID, CF_API_TOKEN, CF_R2_BUCKET)
// These tests verify the API surface, not actual R2 connectivity.

#[test]
fn test_r2_delta_new_with_credentials() {
    let result = R2Delta::new("test-token", "test-account", "test-bucket");
    assert!(result.is_ok());
}

#[test]
fn test_r2_delta_from_env_no_creds() {
    let result = R2Delta::from_env();
    assert!(result.is_err());
}

#[test]
fn test_r2_delta_is_vfs_file_system() {
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    assert_vfs_fs::<R2Delta>();
}

#[test]
fn test_r2_delta_is_delta_store() {
    fn assert_delta<T: DeltaStore>() {}
    assert_delta::<R2Delta>();
}
