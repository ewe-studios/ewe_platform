#![cfg(feature = "vfs-nfs")]

use foundation_nativeapis::native::vfs::nfs::VfsNfs;
use foundation_nativeapis::shared::vfs::{MemoryFs, VfsFileSystem};
use nfsserve::vfs::NFSFileSystem;

#[tokio::test]
async fn test_nfs_root_dir() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    let root = nfs.root_dir().await.unwrap();
    assert_eq!(root.ino, 1);
}

#[tokio::test]
async fn test_nfs_getattr() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    let attr = nfs.getattr(nfsserve::nfs::fileid3::from(1)).await.unwrap();
    assert_eq!(attr.ftype, nfsserve::nfs::ftype3::NF3DIR);
}

#[tokio::test]
async fn test_nfs_capabilities() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    let caps = nfs.capabilities();
    assert!(matches!(caps, nfsserve::vfs::VFSCapabilities::ReadWrite));
}

#[tokio::test]
async fn test_nfs_fs_info() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    let info = nfs.fs_info();
    assert!(info.rtmax > 0);
    assert!(info.wtmax > 0);
}
