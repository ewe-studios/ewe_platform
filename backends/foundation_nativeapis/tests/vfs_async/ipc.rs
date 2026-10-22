//! `VfsClient` over `DirectTransport` to a `VfsDaemon`, through the async traits.

use std::io::SeekFrom;
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::native::vfs::NativeFs;
use foundation_nativeapis::shared::vfs::ipc_client::{DirectTransport, VfsClient};
use foundation_nativeapis::shared::vfs::ipc_daemon::VfsDaemon;
use foundation_nativeapis::shared::vfs::{
    AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem, OpenMode,
};

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vfs_async_ipc_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn client(tmp: &TempDir) -> VfsClient {
    let daemon = VfsDaemon::new(Arc::new(NativeFs::new(&tmp.0).unwrap()));
    VfsClient::new(Arc::new(DirectTransport::new(daemon))).await
}

#[valtron_test]
async fn read_write_and_metadata_round_trip() {
    let tmp = TempDir::new("rw");
    let c = client(&tmp).await;
    let f = c.create_async("/a.txt".into(), 0o644).await.unwrap();
    assert_eq!(f.write_at_async(b"hello".to_vec(), 0).await.unwrap(), 5);
    assert_eq!(f.read_at_async(5, 0).await.unwrap(), b"hello");
    assert_eq!(f.size_async().await.unwrap(), 5);
    assert_eq!(c.read_file_async("/a.txt".into()).await.unwrap(), b"hello");
    assert!(c.capabilities().persistent);
}

/// The daemon now keeps the cursor of a seekable handle: `Current` and
/// `End` are relative to it and to the file size.
#[valtron_test]
async fn seek_from_current_and_end() {
    let tmp = TempDir::new("seek");
    let c = client(&tmp).await;
    c.write_file_async("/s.txt".into(), b"0123456789".to_vec())
        .await
        .unwrap();
    let mut f = c
        .open_seekable_async("/s.txt".into(), OpenMode::Read)
        .await
        .unwrap();

    assert_eq!(f.seek_async(SeekFrom::Start(2)).await.unwrap(), 2);
    assert_eq!(f.seek_async(SeekFrom::Current(3)).await.unwrap(), 5);
    assert_eq!(f.read_async(2).await.unwrap(), b"56");
    assert_eq!(f.seek_async(SeekFrom::End(-1)).await.unwrap(), 9);
    assert!(f.seek_async(SeekFrom::End(-20)).await.is_err());
}

/// Bug fix: a directory handle joined an absolute path to its own path
/// ("/dir//x"). An absolute path is now used as is.
#[valtron_test]
async fn directory_handles_accept_absolute_paths() {
    let tmp = TempDir::new("abs");
    let c = client(&tmp).await;
    c.mkdir_async("/dir".into()).await.unwrap();
    c.write_file_async("/top.txt".into(), b"t".to_vec())
        .await
        .unwrap();

    let dir = c.open_directory_async("/dir".into()).await.unwrap();
    assert!(dir.exists_async("/top.txt".into()).await.unwrap());
    assert_eq!(dir.stat_async("/top.txt".into()).await.unwrap().size, 1);
}

/// Dropping a remote handle releases it in the daemon without a wait, and
/// later requests still get their own responses.
#[valtron_test]
async fn dropped_handles_are_released() {
    let tmp = TempDir::new("release");
    let c = client(&tmp).await;
    for i in 0..3 {
        let f = c.create_async(format!("/f{i}"), 0o644).await.unwrap();
        drop(f);
        let d = c.open_directory_async("/".into()).await.unwrap();
        drop(d);
    }
    let names: Vec<String> = c
        .open_directory_async("/".into())
        .await
        .unwrap()
        .list_async()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["f0", "f1", "f2"]);
}
