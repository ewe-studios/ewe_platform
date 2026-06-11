#![cfg(feature = "vfs-nfs")]

use foundation_nativeapis::native::vfs::nfs::VfsNfs;
use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_nativeapis::shared::vfs::VfsFileSystem;
use nfsserve::nfs::nfsstring;
use nfsserve::vfs::NFSFileSystem;

fn ns(s: &str) -> nfsstring { nfsstring(s.as_bytes().to_vec()) }

#[tokio::test]
async fn test_nfs_root_dir() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    assert_eq!(nfs.root_dir(), 1);
}

#[tokio::test]
async fn test_nfs_capabilities() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    assert!(matches!(nfs.capabilities(), nfsserve::vfs::VFSCapabilities::ReadWrite));
}

#[tokio::test]
async fn test_nfs_getattr_root() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    let attr = nfs.getattr(1).await.unwrap();
    assert!(matches!(attr.ftype, nfsserve::nfs::ftype3::NF3DIR));
}

#[tokio::test]
async fn test_nfs_fsinfo() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    let info = nfs.fsinfo(1).await.unwrap();
    assert!(info.rtmax > 0);
    assert!(info.wtmax > 0);
}

#[tokio::test]
async fn test_nfs_create_and_write() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);

    let (id, _) = nfs.create(1, &ns("test.txt"), nfsserve::nfs::sattr3::default()).await.unwrap();
    assert!(id > 0);

    let data = b"Hello, NFS!";
    nfs.write(id, 0, data).await.unwrap();

    let (read_data, eof) = nfs.read(id, 0, data.len() as u32).await.unwrap();
    assert_eq!(read_data, data);
    assert!(eof);
}

#[tokio::test]
async fn test_nfs_id_to_fh_to_id() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);

    let id: nfsserve::nfs::fileid3 = 42;
    let fh = nfs.id_to_fh(id);
    let recovered_id = nfs.fh_to_id(&fh).unwrap();
    assert_eq!(id, recovered_id);
}

#[tokio::test]
async fn test_nfs_path_to_id() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);

    // Root path should resolve
    let id = nfs.path_to_id(b"/").await;
    assert!(id.is_ok());

    // Non-existent path should fail
    let id = nfs.path_to_id(b"/nonexistent").await;
    assert!(id.is_err());
}

#[tokio::test]
async fn test_nfs_readdir_root() {
    let fs = MemoryFs::new();
    fs.mkdir("/subdir1").unwrap();
    fs.mkdir("/subdir2").unwrap();
    fs.create("/file1.txt", 0o644).unwrap();

    let nfs = VfsNfs::new(fs);
    let dir_id = nfs.path_to_id(b"/").await.unwrap();

    let result = nfs.readdir(dir_id, 0, 10).await.unwrap();
    assert!(result.entries.len() >= 3);
}

#[tokio::test]
async fn test_nfs_symlink() {
    let fs = MemoryFs::new();
    fs.mkdir("/target").unwrap();
    let nfs = VfsNfs::new(fs);

    let result = nfs.symlink(1, &ns("link"), &ns("/target"), &nfsserve::nfs::sattr3::default()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_nfs_remove_file() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);

    // Create a file
    let (id, _) = nfs.create(1, &ns("toremove.txt"), nfsserve::nfs::sattr3::default()).await.unwrap();

    // Remove it
    nfs.remove(1, &ns("toremove.txt")).await.unwrap();
}
