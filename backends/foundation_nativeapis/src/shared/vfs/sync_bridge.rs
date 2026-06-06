//! Synchronous bridge adapters for async VFS traits.
//!
//! **Design:** `&self` methods (`VfsFile`, most of `VfsDirectory`/`VfsFileSystem`)
//! are bridged generically via `SyncFile<A>` using valtron's `exec_async`.
//! Seekable methods (`&mut self`) use `LocalSeekableFile` — plain cursor +
//! delegated I/O — avoiding the dangerous `Arc<Mutex<Option<A>>>` take/put-back
//! pattern that panics on concurrent access.
//!
//! **Deref/Ref patterns:** All wrapper types implement `Deref` to their inner
//! type so they compose naturally with `Arc`, `&self` borrowing, and method
//! delegation. `DerefMut` is only implemented for types with interior mutability
//! (`LocalSeekableFile`).

use std::fmt;
use std::io::SeekFrom;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use super::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use super::error::VfsResult;
use super::exec_async::exec_async;
use super::traits::{DeltaStore, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

/// Wraps an async file for synchronous access via `exec_async`.
///
/// All [`VfsFile`] methods return errors as [`VfsResult`].
pub struct SyncFile<A: AsyncVfsFile + 'static> {
    inner: Arc<A>,
}

impl<A: AsyncVfsFile + 'static> SyncFile<A> {
    /// Creates a new `SyncFile` from a shared async file handle.
    #[must_use]
    pub fn new(inner: Arc<A>) -> Self {
        Self { inner }
    }
}

impl<A: AsyncVfsFile + 'static> fmt::Debug for SyncFile<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncFile").finish_non_exhaustive()
    }
}

impl<A: AsyncVfsFile + 'static> Deref for SyncFile<A> {
    type Target = A;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<A: AsyncVfsFile + 'static> VfsFile for SyncFile<A> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let inner = self.inner.clone();
        let len = buf.len();
        let data = exec_async(async move {
            AsyncVfsFile::read_at_async(&*inner, len, offset).await
        })?;
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let inner = self.inner.clone();
        let data = buf.to_vec();
        exec_async(async move { AsyncVfsFile::write_at_async(&*inner, data, offset).await })
    }

    fn sync_data(&self) -> VfsResult<()> {
        let inner = self.inner.clone();
        exec_async(async move { AsyncVfsFile::sync_data_async(&*inner).await })
    }

    fn size(&self) -> VfsResult<u64> {
        let inner = self.inner.clone();
        exec_async(async move { AsyncVfsFile::size_async(&*inner).await })
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        let inner = self.inner.clone();
        exec_async(async move { AsyncVfsFile::truncate_async(&*inner, size).await })
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        exec_async(async move { AsyncVfsFile::metadata_async(&*inner).await })
    }
}

/// Adds local cursor tracking on top of a bridged [`SyncFile`] to provide seekable access.
///
/// All [`VfsFile`] and [`SeekableVfsFile`] methods return errors as [`VfsResult`].
pub struct LocalSeekableFile<A: AsyncVfsFile + 'static> {
    inner: SyncFile<A>,
    cursor: u64,
}

impl<A: AsyncVfsFile + 'static> LocalSeekableFile<A> {
    /// Creates a new `LocalSeekableFile` with the cursor at position 0.
    #[must_use]
    pub fn new(inner: SyncFile<A>) -> Self {
        Self { inner, cursor: 0 }
    }
}

impl<A: AsyncVfsFile + 'static> fmt::Debug for LocalSeekableFile<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSeekableFile")
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl<A: AsyncVfsFile + 'static> Deref for LocalSeekableFile<A> {
    type Target = SyncFile<A>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<A: AsyncVfsFile + 'static> DerefMut for LocalSeekableFile<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<A: AsyncVfsFile + 'static> VfsFile for LocalSeekableFile<A> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
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

impl<A: AsyncVfsFile + 'static> SeekableVfsFile for LocalSeekableFile<A> {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = self.inner.read_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let n = self.inner.write_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        self.cursor = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => {
                let size = self.inner.size()?;
                if n >= 0 {
                    size.saturating_add(n.unsigned_abs())
                } else {
                    size.saturating_sub(n.unsigned_abs())
                }
            }
            SeekFrom::Current(n) => {
                if n >= 0 {
                    self.cursor.saturating_add(n.unsigned_abs())
                } else {
                    self.cursor.saturating_sub(n.unsigned_abs())
                }
            }
        };
        Ok(self.cursor)
    }

    fn position(&self) -> u64 {
        self.cursor
    }
}

/// Wraps an async directory for synchronous access via `exec_async`.
///
/// All [`VfsDirectory`] methods return errors as [`VfsResult`].
pub struct SyncDirectory<A: AsyncVfsDirectory + 'static> {
    inner: Arc<A>,
    cached_path: String,
}

impl<A: AsyncVfsDirectory + 'static> SyncDirectory<A> {
    /// Creates a new `SyncDirectory` from a shared async directory handle.
    #[must_use]
    pub fn new(inner: Arc<A>) -> Self {
        let cached_path = inner.path();
        Self { inner, cached_path }
    }
}

impl<A: AsyncVfsDirectory + 'static> fmt::Debug for SyncDirectory<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncDirectory")
            .field("path", &self.cached_path)
            .finish_non_exhaustive()
    }
}

impl<A: AsyncVfsDirectory + 'static> Deref for SyncDirectory<A> {
    type Target = A;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<A: AsyncVfsDirectory + 'static> VfsDirectory for SyncDirectory<A> {
    type File = SyncFile<A::File>;
    type SeekableFile = LocalSeekableFile<A::File>;

    fn path(&self) -> &str {
        &self.cached_path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        exec_async(async move { inner.metadata_async().await })
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let inner = self.inner.clone();
        exec_async(async move { inner.list_async().await })
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let inner = self.inner.clone();
        let name = name.to_string();
        exec_async(async move { inner.get_entry_async(name).await })
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let name = name.to_string();
        let file = exec_async(async move { inner.create_file_async(name, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let inner = self.inner.clone();
        let name = name.to_string();
        let dir = exec_async(async move { inner.create_dir_async(name).await })?;
        Ok(Box::new(SyncDynDirectory::<A::File, A::SeekableFile>::new(
            dir,
        )))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let name = name.to_string();
        exec_async(async move { inner.remove_entry_async(name).await })
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let old = old_name.to_string();
        let new = new_name.to_string();
        exec_async(async move { inner.rename_entry_async(old, new).await })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let file = exec_async(async move { inner.open_async(path, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(LocalSeekableFile::new(file))
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let inner = self.inner.clone();
        let path = path.to_string();
        let dir = exec_async(async move { inner.open_directory_async(path).await })?;
        Ok(Box::new(SyncDynDirectory::<A::File, A::SeekableFile>::new(
            dir,
        )))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.stat_async(path).await })
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.exists_async(path).await })
    }
}

struct SyncDynDirectory<AF: AsyncVfsFile + 'static, ASF: AsyncSeekableVfsFile + 'static> {
    inner: Arc<Box<dyn AsyncVfsDirectory<File = AF, SeekableFile = ASF>>>,
    cached_path: String,
}

impl<AF: AsyncVfsFile + 'static, ASF: AsyncSeekableVfsFile + 'static>
    SyncDynDirectory<AF, ASF>
{
    fn new(inner: Box<dyn AsyncVfsDirectory<File = AF, SeekableFile = ASF>>) -> Self {
        let cached_path = inner.path();
        Self {
            inner: Arc::new(inner),
            cached_path,
        }
    }
}

impl<AF: AsyncVfsFile + 'static, ASF: AsyncSeekableVfsFile + 'static> fmt::Debug
    for SyncDynDirectory<AF, ASF>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncDynDirectory")
            .field("path", &self.cached_path)
            .finish_non_exhaustive()
    }
}

impl<AF: AsyncVfsFile + 'static, ASF: AsyncSeekableVfsFile + 'static> Deref
    for SyncDynDirectory<AF, ASF>
{
    type Target = Box<dyn AsyncVfsDirectory<File = AF, SeekableFile = ASF>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<AF: AsyncVfsFile + 'static, ASF: AsyncSeekableVfsFile + 'static> VfsDirectory
    for SyncDynDirectory<AF, ASF>
{
    type File = SyncFile<AF>;
    type SeekableFile = LocalSeekableFile<AF>;

    fn path(&self) -> &str {
        &self.cached_path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        exec_async(async move { inner.metadata_async().await })
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let inner = self.inner.clone();
        exec_async(async move { inner.list_async().await })
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let inner = self.inner.clone();
        let name = name.to_string();
        exec_async(async move { inner.get_entry_async(name).await })
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let name = name.to_string();
        let file = exec_async(async move { inner.create_file_async(name, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let inner = self.inner.clone();
        let name = name.to_string();
        let dir = exec_async(async move { inner.create_dir_async(name).await })?;
        Ok(Box::new(SyncDynDirectory::<AF, ASF>::new(dir)))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let name = name.to_string();
        exec_async(async move { inner.remove_entry_async(name).await })
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let old = old_name.to_string();
        let new = new_name.to_string();
        exec_async(async move { inner.rename_entry_async(old, new).await })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let file = exec_async(async move { inner.open_async(path, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(LocalSeekableFile::new(file))
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let inner = self.inner.clone();
        let path = path.to_string();
        let dir = exec_async(async move { inner.open_directory_async(path).await })?;
        Ok(Box::new(SyncDynDirectory::<AF, ASF>::new(dir)))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.stat_async(path).await })
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.exists_async(path).await })
    }
}

/// Wraps an async filesystem for synchronous access via `exec_async`.
///
/// All [`VfsFileSystem`] methods return errors as [`VfsResult`].
pub struct SyncFs<A: AsyncVfsFileSystem + 'static> {
    inner: Arc<A>,
}

impl<A: AsyncVfsFileSystem + 'static> SyncFs<A> {
    /// Creates a new `SyncFs` by wrapping an owned async filesystem.
    #[must_use]
    pub fn new(inner: A) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Creates a new `SyncFs` from a pre-existing `Arc`.
    #[must_use]
    pub fn from_arc(inner: Arc<A>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the wrapped inner filesystem.
    /// Useful for backends that want to delegate to `SyncFs` while
    /// overriding specific methods (e.g. `open_seekable`).
    #[must_use]
    pub fn inner(&self) -> &Arc<A> {
        &self.inner
    }
}

impl<A: AsyncVfsFileSystem + 'static> fmt::Debug for SyncFs<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncFs").finish_non_exhaustive()
    }
}

impl<A: AsyncVfsFileSystem + 'static> Deref for SyncFs<A> {
    type Target = A;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<A: AsyncVfsFileSystem + 'static> VfsFileSystem for SyncFs<A> {
    type File = SyncFile<A::File>;
    type SeekableFile = LocalSeekableFile<A::File>;
    type Directory = SyncDirectory<A::Directory>;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.stat_async(path).await })
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.exists_async(path).await })
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.chmod_async(path, mode).await })
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let target = target.to_string();
        let link = link.to_string();
        exec_async(async move { inner.symlink_async(target, link).await })
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.readlink_async(path).await })
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let from = from.to_string();
        let to = to.to_string();
        exec_async(async move { inner.rename_async(from, to).await })
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.remove_async(path).await })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let file = exec_async(async move { inner.open_async(path, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(LocalSeekableFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let dir = exec_async(async move { inner.open_directory_async(path).await })?;
        Ok(SyncDirectory::new(Arc::new(dir)))
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let file = exec_async(async move { inner.create_async(path, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.mkdir_async(path).await })
    }
}

impl<A: AsyncDeltaStore + 'static> DeltaStore for SyncFs<A> {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.add_whiteout_async(path, version).await })
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.is_whiteout_async(path).await })
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.remove_whiteout_async(path).await })
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        let inner = self.inner.clone();
        let dir = dir.to_string();
        exec_async(async move { inner.list_whiteouts_async(dir).await })
    }

    fn flush(&self) -> VfsResult<()> {
        let inner = self.inner.clone();
        exec_async(async move { inner.flush_async().await })
    }

    fn reset(&self) -> VfsResult<()> {
        let inner = self.inner.clone();
        exec_async(async move { inner.reset_async().await })
    }
}
