#![cfg(feature = "vfs-d1")]

use foundation_nativeapis::shared::vfs::d1_delta::D1Delta;
use foundation_nativeapis::shared::vfs::{
    DeltaStore, OpenMode, VfsFile, VfsFileSystem,
};

// Note: D1Delta requires Cloudflare credentials (CF_ACCOUNT_ID, CF_API_TOKEN, CF_D1_DATABASE_ID)
// These tests verify the API surface, not actual D1 connectivity.

#[test]
fn test_d1_delta_new_with_credentials() {
    // Verify constructor works with test credentials
    let result = D1Delta::new("test-token", "test-account", "test-db");
    assert!(result.is_ok());
}

#[test]
fn test_d1_delta_from_env_no_creds() {
    // Without env vars, from_env should fail
    let result = D1Delta::from_env();
    assert!(result.is_err());
}

#[test]
fn test_d1_delta_is_vfs_file_system() {
    // Verify D1Delta implements VfsFileSystem (compile-time check)
    fn assert_vfs_fs<T: VfsFileSystem>() {}
    assert_vfs_fs::<D1Delta>();
}

#[test]
fn test_d1_delta_is_delta_store() {
    // Verify D1Delta implements DeltaStore (compile-time check)
    fn assert_delta<T: DeltaStore>() {}
    assert_delta::<D1Delta>();
}
