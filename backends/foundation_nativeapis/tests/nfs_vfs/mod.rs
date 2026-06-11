#![cfg(feature = "vfs-nfs")]

use foundation_nativeapis::native::vfs::nfs::VfsNfs;
use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_nativeapis::shared::vfs::VfsFileSystem;
use foundation_core::valtron::{collect_one, execute, from_future, initialize_pool, PoolGuard};
use nfsserve::nfs::{nfsstat3, nfsstring};
use nfsserve::vfs::NFSFileSystem;
use std::sync::Arc;

fn ns(s: &str) -> nfsstring { nfsstring(s.as_bytes().to_vec()) }

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Run an async NFS operation through valtron's executor.
/// NFS operations return `Result<T, nfsstat3>` where nfsstat3 is an NFS status code enum.
fn run_nfs_async<T: Send + 'static, F>(future: F) -> Result<T, nfsstat3>
where
    F: std::future::Future<Output = Result<T, nfsstat3>> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("execute failed");
    let result: Option<Result<T, nfsstat3>> = collect_one(stream);
    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => panic!("no result from valtron executor"),
    }
}

// ── Static Tests (no valtron needed) ──

#[test]
fn test_nfs_root_dir() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    assert_eq!(nfs.root_dir(), 1);
}

#[test]
fn test_nfs_capabilities() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);
    assert!(matches!(nfs.capabilities(), nfsserve::vfs::VFSCapabilities::ReadWrite));
}

#[test]
fn test_nfs_id_to_fh_to_id() {
    let fs = MemoryFs::new();
    let nfs = VfsNfs::new(fs);

    let id: nfsserve::nfs::fileid3 = 42;
    let fh = nfs.id_to_fh(id);
    let recovered_id = nfs.fh_to_id(&fh).unwrap();
    assert_eq!(id, recovered_id);
}

// ── Async Tests through valtron ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_getattr_root() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    let nfs = Arc::new(VfsNfs::new(fs));

    let attr = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.getattr(1).await }
    }).unwrap();
    assert!(matches!(attr.ftype, nfsserve::nfs::ftype3::NF3DIR));
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_fsinfo() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    let nfs = Arc::new(VfsNfs::new(fs));

    let info = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.fsinfo(1).await }
    }).unwrap();
    assert!(info.rtmax > 0);
    assert!(info.wtmax > 0);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_create_and_write() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    let nfs = Arc::new(VfsNfs::new(fs));

    let (id, _) = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.create(1, &ns("test.txt"), nfsserve::nfs::sattr3::default()).await }
    }).unwrap();
    assert!(id > 0);

    let data = b"Hello, NFS!";
    run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.write(id, 0, data).await }
    }).unwrap();

    let (read_data, eof) = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.read(id, 0, data.len() as u32).await }
    }).unwrap();
    assert_eq!(read_data, data);
    assert!(eof);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_path_to_id() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    let nfs = Arc::new(VfsNfs::new(fs));

    let result = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.path_to_id(b"/").await }
    });
    assert!(result.is_ok());

    let result = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.path_to_id(b"/nonexistent").await }
    });
    assert!(result.is_err());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_readdir_root() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    fs.mkdir("/subdir1").unwrap();
    fs.mkdir("/subdir2").unwrap();
    fs.create("/file1.txt", 0o644).unwrap();

    let nfs = Arc::new(VfsNfs::new(fs));
    let dir_id = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.path_to_id(b"/").await }
    }).unwrap();

    let result = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.readdir(dir_id, 0, 10).await }
    }).unwrap();
    assert!(result.entries.len() >= 3);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_symlink() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    fs.mkdir("/target").unwrap();
    let nfs = Arc::new(VfsNfs::new(fs));

    let result = run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.symlink(1, &ns("link"), &ns("/target"), &nfsserve::nfs::sattr3::default()).await }
    });
    assert!(result.is_ok());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_nfs_remove_file() {
    let _guard = init_pool();
    let fs = MemoryFs::new();
    let nfs = Arc::new(VfsNfs::new(fs));

    run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.create(1, &ns("toremove.txt"), nfsserve::nfs::sattr3::default()).await }
    }).unwrap();
    run_nfs_async({
        let nfs = nfs.clone();
        async move { nfs.remove(1, &ns("toremove.txt")).await }
    }).unwrap();
}
