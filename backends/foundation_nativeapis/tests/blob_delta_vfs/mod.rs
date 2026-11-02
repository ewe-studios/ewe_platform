#![cfg(feature = "vfs-d1")]

//! `BlobDelta` (the core behind `D1Delta` / `R2Delta`) over the in-memory
//! blob store, so the delta's behaviour is tested without a Cloudflare
//! emulator: async directly, and sync through `SyncFs`.

use std::io::SeekFrom;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use foundation_core::valtron::{initialize_pool, valtron_test, PoolGuard};
use foundation_db::traits::BlobListStream;
use foundation_db::{BlobStore, MemoryStorage, StorageResult};
use foundation_nativeapis::native::vfs::blob_delta::{BlobDelta, BlobDeltaConfig};
use foundation_nativeapis::shared::vfs::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
use foundation_nativeapis::shared::vfs::{DeltaStore, OpenMode, VfsFileSystem, VfsFileType};

fn delta() -> BlobDelta<MemoryStorage> {
    BlobDelta::from_store(MemoryStorage::new())
}

/// A blob store that counts every byte passed to `put_blob`.
#[derive(Clone)]
struct CountingStore {
    inner: MemoryStorage,
    written: Arc<AtomicUsize>,
}

impl CountingStore {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let written = Arc::new(AtomicUsize::new(0));
        (
            Self {
                inner: MemoryStorage::new(),
                written: Arc::clone(&written),
            },
            written,
        )
    }
}

#[async_trait]
impl BlobStore for CountingStore {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.written.fetch_add(data.len(), Ordering::Relaxed);
        self.inner.put_blob(key, data).await
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.inner.get_blob(key).await
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        self.inner.delete_blob(key).await
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        self.inner.blob_exists(key).await
    }

    async fn list_blobs_after(
        &self,
        prefix: &str,
        start_after: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<BlobListStream> {
        self.inner.list_blobs_after(prefix, start_after, limit).await
    }
}

#[valtron_test]
async fn create_write_read_round_trip() {
    let d = delta();
    let f = d.create_async("/a.txt".into(), 0o600).await.unwrap();
    assert_eq!(f.write_at_async(b"hello".to_vec(), 0).await.unwrap(), 5);
    assert_eq!(d.read_file_async("/a.txt".into()).await.unwrap(), b"hello");

    let meta = d.stat_async("/a.txt".into()).await.unwrap();
    assert_eq!(meta.size, 5);
    assert_eq!(meta.permissions, 0o600);
    assert_eq!(meta.file_type, VfsFileType::Regular);
}

#[valtron_test]
async fn write_at_honours_the_offset() {
    let d = delta();
    let f = d.create_async("/f".into(), 0o644).await.unwrap();
    f.write_at_async(b"hello world".to_vec(), 0).await.unwrap();
    f.write_at_async(b"WORLD".to_vec(), 6).await.unwrap();
    // Past the end pads with zeros.
    f.write_at_async(b"!".to_vec(), 12).await.unwrap();
    assert_eq!(d.read_file_async("/f".into()).await.unwrap(), b"hello WORLD\0!");
    assert_eq!(f.read_at_async(5, 6).await.unwrap(), b"WORLD");
}

#[valtron_test]
async fn read_only_files_reject_writes_and_truncate() {
    let d = delta();
    d.create_async("/ro".into(), 0o644).await.unwrap();
    let f = d.open_async("/ro".into(), OpenMode::Read).await.unwrap();
    assert!(f.write_at_async(b"x".to_vec(), 0).await.is_err());
    assert!(f.truncate_async(0).await.is_err());
}

#[valtron_test]
async fn truncate_persists() {
    let d = delta();
    let f = d.create_async("/t".into(), 0o644).await.unwrap();
    f.write_at_async(b"abcdef".to_vec(), 0).await.unwrap();
    f.truncate_async(3).await.unwrap();
    assert_eq!(d.read_file_async("/t".into()).await.unwrap(), b"abc");
    assert_eq!(d.stat_async("/t".into()).await.unwrap().size, 3);
}

#[valtron_test]
async fn seekable_file_tracks_its_cursor() {
    let d = delta();
    d.create_async("/s".into(), 0o644).await.unwrap();
    let mut f = d.open_seekable_async("/s".into(), OpenMode::ReadWrite).await.unwrap();
    f.write_async(b"abc".to_vec()).await.unwrap();
    f.write_async(b"def".to_vec()).await.unwrap();
    assert_eq!(f.position_async(), 6);
    assert_eq!(f.seek_async(SeekFrom::Start(1)).await.unwrap(), 1);
    assert_eq!(f.read_async(2).await.unwrap(), b"bc");
    assert_eq!(f.seek_async(SeekFrom::End(-1)).await.unwrap(), 5);
    assert_eq!(f.read_async(10).await.unwrap(), b"f");
    assert!(f.seek_async(SeekFrom::Current(-100)).await.is_err());
}

#[valtron_test]
async fn open_missing_file_is_not_found() {
    let d = delta();
    assert!(d.open_async("/nope".into(), OpenMode::Read).await.is_err());
    assert!(!d.exists_async("/nope".into()).await.unwrap());
}

#[valtron_test]
async fn remove_whites_out_and_create_clears_it() {
    let d = delta();
    d.create_async("/w".into(), 0o644).await.unwrap();
    d.remove_async("/w".into()).await.unwrap();
    assert!(!d.exists_async("/w".into()).await.unwrap());
    assert_eq!(d.is_whiteout_async("/w".into()).await.unwrap(), Some(0));

    d.create_async("/w".into(), 0o644).await.unwrap();
    assert!(d.exists_async("/w".into()).await.unwrap());
    assert_eq!(d.is_whiteout_async("/w".into()).await.unwrap(), None);
}

#[valtron_test]
async fn whiteouts_keep_their_version() {
    let d = delta();
    d.add_whiteout_async("/v".into(), 42).await.unwrap();
    assert_eq!(d.is_whiteout_async("/v".into()).await.unwrap(), Some(42));
    d.remove_whiteout_async("/v".into()).await.unwrap();
    assert_eq!(d.is_whiteout_async("/v".into()).await.unwrap(), None);
}

#[valtron_test]
async fn rename_moves_data_and_metadata() {
    let d = delta();
    let f = d.create_async("/from".into(), 0o640).await.unwrap();
    f.write_at_async(b"data".to_vec(), 0).await.unwrap();
    d.rename_async("/from".into(), "/to".into()).await.unwrap();
    assert!(!d.exists_async("/from".into()).await.unwrap());
    assert_eq!(d.read_file_async("/to".into()).await.unwrap(), b"data");
    assert_eq!(d.stat_async("/to".into()).await.unwrap().permissions, 0o640);
    assert!(d.rename_async("/missing".into(), "/x".into()).await.is_err());
}

#[valtron_test]
async fn directories_create_and_look_up_children() {
    let d = delta();
    d.mkdir_async("/dir".into()).await.unwrap();
    let dir = d.open_directory_async("/dir".into()).await.unwrap();
    dir.create_file_async("child".into(), 0o644).await.unwrap();

    let entry = dir.get_entry_async("child".into()).await.unwrap().expect("child");
    assert_eq!(entry.name, "child");
    assert_eq!(entry.file_type, VfsFileType::Regular);
    assert!(dir.get_entry_async("other".into()).await.unwrap().is_none());

    // A file is not a directory.
    assert!(d.open_directory_async("/dir/child".into()).await.is_err());
}

#[valtron_test]
async fn blob_delta_lists_files_and_whiteouts() {
    let d = delta();
    d.mkdir_async("/d".into()).await.unwrap();
    d.create_async("/d/a.txt".into(), 0o644).await.unwrap();
    d.create_async("/d/b.txt".into(), 0o644).await.unwrap();
    d.mkdir_async("/d/sub".into()).await.unwrap();
    d.create_async("/d/gone.txt".into(), 0o644).await.unwrap();
    d.remove_async("/d/gone.txt".into()).await.unwrap();
    d.add_whiteout_async("/d/removed".into(), 5).await.unwrap();

    let dir = d.open_directory_async("/d".into()).await.unwrap();
    let mut names: Vec<String> = dir
        .list_async()
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["a.txt", "b.txt", "sub"]);

    let mut whiteouts = d.list_whiteouts_async("/d".into()).await.unwrap();
    whiteouts.sort();
    assert_eq!(
        whiteouts,
        vec![
            ("/d/gone.txt".to_string(), 0),
            ("/d/removed".to_string(), 5),
        ]
    );
}

#[valtron_test]
async fn blob_delta_reset_clears_everything() {
    let d = delta();
    d.mkdir_async("/d".into()).await.unwrap();
    d.create_async("/d/a".into(), 0o644).await.unwrap();
    d.add_whiteout_async("/w".into(), 1).await.unwrap();

    d.reset_async().await.unwrap();

    assert!(!d.exists_async("/d/a".into()).await.unwrap());
    assert!(!d.exists_async("/d".into()).await.unwrap());
    assert!(d.list_whiteouts_async("/".into()).await.unwrap().is_empty());
}

#[valtron_test]
async fn blob_delta_small_writes_touch_one_chunk() {
    let (store, written) = CountingStore::new();
    let config = BlobDeltaConfig::default().with_chunk_size(4096);
    let d = BlobDelta::from_store_with_config(store, config);
    let f = d.create_async("/big".into(), 0o644).await.unwrap();

    // 1 MiB in 4 KiB pieces. Chunked writes touch one 4 KiB chunk per write,
    // so the store sees about 1 MiB plus metadata; a whole-file rewrite would
    // push ~128 MiB through the store.
    let piece = vec![0xabu8; 4096];
    for i in 0..256usize {
        f.write_at_async(piece.clone(), (i * 4096) as u64)
            .await
            .unwrap();
    }

    assert_eq!(
        d.read_file_async("/big".into()).await.unwrap().len(),
        1024 * 1024
    );
    let bytes = written.load(Ordering::Relaxed);
    assert!(
        bytes < 16 * 1024 * 1024,
        "expected chunked writes, but {bytes} bytes reached the store"
    );
}

#[valtron_test]
async fn chmod_updates_permissions() {
    let d = delta();
    d.create_async("/c".into(), 0o644).await.unwrap();
    d.chmod_async("/c".into(), 0o600).await.unwrap();
    assert_eq!(d.stat_async("/c".into()).await.unwrap().permissions, 0o600);
}

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

#[test]
fn sync_fs_drives_the_async_delta() {
    let _guard = init_pool();
    let sync = SyncFs::new(delta());
    sync.create("/sync.txt", 0o644).unwrap();
    sync.write_file("/sync.txt", b"via SyncFs").unwrap();
    assert_eq!(sync.read_file("/sync.txt").unwrap(), b"via SyncFs");
    sync.add_whiteout("/sync.txt", 3).unwrap();
    assert_eq!(sync.is_whiteout("/sync.txt").unwrap(), Some(3));
}
