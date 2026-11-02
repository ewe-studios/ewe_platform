//! Tests that the VFS `copy` and `copy_async` default methods stream in chunks.
//!
//! A counting VFS wraps `MemoryFs`, overrides `copy_chunk_size` to a small
//! value, and records every read size. The copy must issue several reads and
//! no single read may exceed the configured chunk size.

#![cfg(feature = "vfs")]

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::shared::vfs::{
    AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem, MemoryFs, OpenMode,
    SeekableVfsFile, VfsCapabilities, VfsDirEntry, VfsDirectory, VfsFile, VfsFileSystem,
    VfsMetadata, VfsResult,
};

/// The chunk size the counting wrappers report. Small so the test stays fast.
const CHUNK: usize = 256;

// ── Sync counting VFS ──

struct CountingFile<F> {
    inner: F,
    reads: Arc<Mutex<Vec<usize>>>,
}

impl<F: VfsFile> VfsFile for CountingFile<F> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.reads.lock().unwrap().push(buf.len());
        self.inner.read_at(buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.inner.write_at(buf, offset)
    }

    fn sync_data(&self) -> VfsResult<()> {
        self.inner.sync_data()
    }

    fn size(&self) -> VfsResult<u64> {
        self.inner.size()
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate(size)
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata()
    }
}

struct StubDir<F, S>(PhantomData<(F, S)>);

impl<F: VfsFile, S: SeekableVfsFile> VfsDirectory for StubDir<F, S> {
    type File = F;
    type SeekableFile = S;

    fn path(&self) -> &str {
        unimplemented!()
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        unimplemented!()
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        unimplemented!()
    }

    fn get_entry(&self, _name: &str) -> VfsResult<Option<VfsDirEntry>> {
        unimplemented!()
    }

    fn create_file(&self, _name: &str, _mode: u32) -> VfsResult<Self::File> {
        unimplemented!()
    }

    fn create_dir(
        &self,
        _name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        unimplemented!()
    }

    fn remove_entry(&self, _name: &str) -> VfsResult<()> {
        unimplemented!()
    }

    fn rename_entry(&self, _old_name: &str, _new_name: &str) -> VfsResult<()> {
        unimplemented!()
    }

    fn open(&self, _path: &str, _mode: OpenMode) -> VfsResult<Self::File> {
        unimplemented!()
    }

    fn open_seekable(&self, _path: &str, _mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        unimplemented!()
    }

    fn open_directory(
        &self,
        _path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        unimplemented!()
    }

    fn stat(&self, _path: &str) -> VfsResult<VfsMetadata> {
        unimplemented!()
    }

    fn exists(&self, _path: &str) -> VfsResult<bool> {
        unimplemented!()
    }
}

struct CountingFs<F> {
    inner: F,
    reads: Arc<Mutex<Vec<usize>>>,
}

impl<F: VfsFileSystem> CountingFs<F> {
    fn new(inner: F) -> (Self, Arc<Mutex<Vec<usize>>>) {
        let reads = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner,
                reads: reads.clone(),
            },
            reads,
        )
    }
}

impl<F: VfsFileSystem> VfsFileSystem for CountingFs<F> {
    type File = CountingFile<F::File>;
    type SeekableFile = F::SeekableFile;
    type Directory = StubDir<CountingFile<F::File>, F::SeekableFile>;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        self.inner.stat(path)
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        self.inner.exists(path)
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        self.inner.chmod(path, mode)
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        self.inner.symlink(target, link)
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        self.inner.readlink(path)
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        self.inner.rename(from, to)
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        self.inner.remove(path)
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let file = self.inner.open(path, mode)?;
        Ok(CountingFile {
            inner: file,
            reads: self.reads.clone(),
        })
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        self.inner.open_seekable(path, mode)
    }

    fn open_directory(&self, _path: &str) -> VfsResult<Self::Directory> {
        unimplemented!("directory is not used by copy")
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let file = self.inner.create(path, mode)?;
        Ok(CountingFile {
            inner: file,
            reads: self.reads.clone(),
        })
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        self.inner.mkdir(path)
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        self.inner.inode(path)
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.inner.path_by_inode(ino)
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.inner.stat_by_inode(ino)
    }

    fn copy_chunk_size(&self) -> usize {
        CHUNK
    }
}

// ── Async counting VFS ──

struct CountingAsyncFile<F> {
    inner: F,
    reads: Arc<Mutex<Vec<usize>>>,
}

#[async_trait::async_trait]
impl<F: AsyncVfsFile> AsyncVfsFile for CountingAsyncFile<F> {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        self.reads.lock().unwrap().push(len);
        self.inner.read_at_async(len, offset).await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        self.inner.write_at_async(data, offset).await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        self.inner.sync_data_async().await
    }

    async fn size_async(&self) -> VfsResult<u64> {
        self.inner.size_async().await
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate_async(size).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata_async().await
    }
}

struct StubAsyncDir<F, S>(PhantomData<(F, S)>);

#[async_trait::async_trait]
impl<F: AsyncVfsFile + 'static, S: AsyncSeekableVfsFile + 'static> AsyncVfsDirectory
    for StubAsyncDir<F, S>
{
    type File = F;
    type SeekableFile = S;

    fn path(&self) -> String {
        unimplemented!()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        unimplemented!()
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        unimplemented!()
    }

    async fn get_entry_async(&self, _name: String) -> VfsResult<Option<VfsDirEntry>> {
        unimplemented!()
    }

    async fn create_file_async(&self, _name: String, _mode: u32) -> VfsResult<Self::File> {
        unimplemented!()
    }

    async fn create_dir_async(
        &self,
        _name: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        unimplemented!()
    }

    async fn remove_entry_async(&self, _name: String) -> VfsResult<()> {
        unimplemented!()
    }

    async fn rename_entry_async(&self, _old_name: String, _new_name: String) -> VfsResult<()> {
        unimplemented!()
    }

    async fn open_async(&self, _path: String, _mode: OpenMode) -> VfsResult<Self::File> {
        unimplemented!()
    }

    async fn open_seekable_async(
        &self,
        _path: String,
        _mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        unimplemented!()
    }

    async fn open_directory_async(
        &self,
        _path: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        unimplemented!()
    }

    async fn stat_async(&self, _path: String) -> VfsResult<VfsMetadata> {
        unimplemented!()
    }

    async fn exists_async(&self, _path: String) -> VfsResult<bool> {
        unimplemented!()
    }
}

struct CountingAsyncFs<F> {
    inner: F,
    reads: Arc<Mutex<Vec<usize>>>,
}

impl<F: AsyncVfsFileSystem> CountingAsyncFs<F> {
    fn new(inner: F) -> (Self, Arc<Mutex<Vec<usize>>>) {
        let reads = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner,
                reads: reads.clone(),
            },
            reads,
        )
    }
}

#[async_trait::async_trait]
impl<F: AsyncVfsFileSystem> AsyncVfsFileSystem for CountingAsyncFs<F> {
    type File = CountingAsyncFile<F::File>;
    type SeekableFile = F::SeekableFile;
    type Directory = StubAsyncDir<CountingAsyncFile<F::File>, F::SeekableFile>;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.inner.stat_async(path).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.inner.exists_async(path).await
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        self.inner.chmod_async(path, mode).await
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        self.inner.symlink_async(target, link).await
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        self.inner.readlink_async(path).await
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        self.inner.rename_async(from, to).await
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        self.inner.remove_async(path).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        let file = self.inner.open_async(path, mode).await?;
        Ok(CountingAsyncFile {
            inner: file,
            reads: self.reads.clone(),
        })
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        self.inner.open_seekable_async(path, mode).await
    }

    async fn open_directory_async(&self, _path: String) -> VfsResult<Self::Directory> {
        unimplemented!("directory is not used by copy")
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        let file = self.inner.create_async(path, mode).await?;
        Ok(CountingAsyncFile {
            inner: file,
            reads: self.reads.clone(),
        })
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        self.inner.mkdir_async(path).await
    }

    fn copy_chunk_size(&self) -> usize {
        CHUNK
    }
}

// ── Tests ──

#[test]
fn vfs_copy_streams_in_chunks() {
    let content: Vec<u8> = (0..1000u16).map(|b| (b % 251) as u8).collect();

    let inner = MemoryFs::new();
    inner.write_file("/src", &content).unwrap();

    let (fs, reads) = CountingFs::new(inner);
    fs.copy("/src", "/dst").unwrap();

    let recorded = reads.lock().unwrap().clone();

    assert_eq!(fs.read_file("/dst").unwrap(), content);

    assert!(
        recorded.len() > 1,
        "copy issued only one read: {recorded:?}"
    );
    assert_eq!(
        recorded.iter().sum::<usize>(),
        content.len(),
        "copy must read the whole file"
    );
    for read in recorded {
        assert!(
            read <= CHUNK,
            "read of {read} bytes exceeds the {CHUNK}-byte chunk"
        );
    }
}

#[valtron_test]
async fn vfs_copy_async_streams_in_chunks() {
    let content: Vec<u8> = (0..1000u16).map(|b| (b % 251) as u8).collect();

    let inner = MemoryFs::new();
    inner
        .write_file_async("/src".to_string(), content.clone())
        .await
        .unwrap();

    let (fs, reads) = CountingAsyncFs::new(inner);
    fs.copy_async("/src".to_string(), "/dst".to_string())
        .await
        .unwrap();

    let recorded = reads.lock().unwrap().clone();

    assert_eq!(
        fs.read_file_async("/dst".to_string()).await.unwrap(),
        content
    );

    assert!(
        recorded.len() > 1,
        "copy issued only one read: {recorded:?}"
    );
    assert_eq!(
        recorded.iter().sum::<usize>(),
        content.len(),
        "copy must read the whole file"
    );
    for read in recorded {
        assert!(
            read <= CHUNK,
            "read of {read} bytes exceeds the {CHUNK}-byte chunk"
        );
    }
}
