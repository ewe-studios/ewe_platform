//! A delta store over any async blob store (Cloudflare D1, R2).
//!
//! WHY: the D1 and R2 deltas used to reach their stores through the sync
//! `Blocking` view and expose async VFS traits that blocked inside async code.
//! The stores are async-only, so the delta is too: [`BlobDelta`] implements
//! [`AsyncVfsFileSystem`] and [`AsyncDeltaStore`] by awaiting
//! [`BlobStore`]. Sync callers wrap it in
//! [`SyncFs`](crate::shared::vfs::sync_bridge::SyncFs), the VFS's sync
//! boundary.
//!
//! WHAT — storage layout (one blob per key):
//! - `vfs/data/{path}`: the file's content;
//! - `vfs/meta/{path}`: its `VfsMetadata` as JSON;
//! - `vfs/whiteout/{path}`: a whiteout tombstone (the version, little-endian
//!   `u64`).
//!
//! HOW: a file is one blob, so a write reads the blob, splices the bytes in at
//! the offset and writes it back. Blob stores can't list keys, so directory
//! listing and `list_whiteouts` return nothing (backlog §13).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::SeekFrom;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use foundation_db::{BlobStore, StorageError};
use foundation_errstacks::ErrorTrace;

use crate::shared::vfs::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::types::{
    Checksum, OpenMode, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType, VfsMetadata,
};

/// The boxed directory type the async directory trait hands back.
type BoxedDir<S> = Box<dyn AsyncVfsDirectory<File = BlobFile<S>, SeekableFile = BlobFile<S>>>;

// By value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn serr(e: StorageError) -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::Backend {
        message: e.to_string(),
    })
}

fn not_found(path: &str) -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::NotFound {
        path: path.to_string(),
    })
}

fn unsupported(operation: &str) -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::Unsupported {
        operation: operation.to_string(),
    })
}

fn to_usize(n: u64) -> VfsResult<usize> {
    usize::try_from(n).map_err(|_| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("offset {n} does not fit in memory"),
        })
    })
}

fn join(dir: &str, name: &str) -> String {
    if name.starts_with('/') {
        name.to_string()
    } else if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn path_hash(path: &str) -> u64 {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    h.finish()
}

fn new_meta(path: &str, size: u64, file_type: VfsFileType, permissions: u32) -> VfsMetadata {
    let now = SystemTime::now();
    VfsMetadata {
        inode: path_hash(path),
        size,
        file_type,
        permissions,
        owner: (0, 0),
        created: Some(now),
        modified: Some(now),
        accessed: Some(now),
        checksum: Checksum::None,
        version: 0,
        state: VfsEntryState::Ready,
    }
}

/// A delta store over an async blob store. See the module docs.
pub struct BlobDelta<S: BlobStore + 'static> {
    store: Arc<S>,
    capabilities: VfsCapabilities,
}

impl<S: BlobStore + 'static> Clone for BlobDelta<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            capabilities: self.capabilities,
        }
    }
}

impl<S: BlobStore + 'static> BlobDelta<S> {
    /// A delta over `store`.
    #[must_use]
    pub fn from_store(store: S) -> Self {
        Self {
            store: Arc::new(store),
            capabilities: VfsCapabilities {
                seekable: true,
                symlinks: false,
                permissions_enforced: false,
                event_emission: false,
                persistent: true,
            },
        }
    }

    /// The underlying store.
    #[must_use]
    pub fn store(&self) -> &S {
        &self.store
    }

    fn data_key(path: &str) -> String {
        format!("vfs/data/{}", path.trim_start_matches('/'))
    }

    fn meta_key(path: &str) -> String {
        format!("vfs/meta/{}", path.trim_start_matches('/'))
    }

    fn whiteout_key(path: &str) -> String {
        format!("vfs/whiteout/{}", path.trim_start_matches('/'))
    }

    async fn read_data(&self, path: &str) -> VfsResult<Option<Vec<u8>>> {
        self.store.get_blob_async(&Self::data_key(path)).await.map_err(serr)
    }

    async fn get_meta(&self, path: &str) -> VfsResult<Option<VfsMetadata>> {
        match self.store.get_blob_async(&Self::meta_key(path)).await.map_err(serr)? {
            Some(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| {
                ErrorTrace::new(VfsError::Backend {
                    message: format!("meta deserialize: {e}"),
                })
            }),
            None => Ok(None),
        }
    }

    async fn store_meta(&self, path: &str, meta: &VfsMetadata) -> VfsResult<()> {
        let bytes = serde_json::to_vec(meta).map_err(|e| {
            ErrorTrace::new(VfsError::Backend {
                message: format!("meta serialize: {e}"),
            })
        })?;
        self.store
            .put_blob_async(&Self::meta_key(path), &bytes)
            .await
            .map_err(serr)
    }

    /// Replace a file's content and bring its metadata (size, mtime) in line.
    async fn write_data(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        self.store
            .put_blob_async(&Self::data_key(path), data)
            .await
            .map_err(serr)?;
        let mut meta = self
            .get_meta(path)
            .await?
            .unwrap_or_else(|| new_meta(path, 0, VfsFileType::Regular, 0o644));
        meta.size = data.len() as u64;
        meta.modified = Some(SystemTime::now());
        self.store_meta(path, &meta).await
    }

    async fn whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        match self
            .store
            .get_blob_async(&Self::whiteout_key(path))
            .await
            .map_err(serr)?
        {
            Some(bytes) => {
                let version = bytes
                    .get(..8)
                    .and_then(|b| <[u8; 8]>::try_from(b).ok())
                    .map_or(0, u64::from_le_bytes);
                Ok(Some(version))
            }
            None => Ok(None),
        }
    }

    async fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        if self.whiteout(path).await?.is_some() {
            return Err(not_found(path));
        }
        if let Some(meta) = self.get_meta(path).await? {
            return Ok(meta);
        }
        match self.read_data(path).await? {
            Some(data) => Ok(new_meta(path, data.len() as u64, VfsFileType::Regular, 0o644)),
            None => Err(not_found(path)),
        }
    }

    async fn exists(&self, path: &str) -> VfsResult<bool> {
        if self.whiteout(path).await?.is_some() {
            return Ok(false);
        }
        if self
            .store
            .blob_exists_async(&Self::meta_key(path))
            .await
            .map_err(serr)?
        {
            return Ok(true);
        }
        self.store
            .blob_exists_async(&Self::data_key(path))
            .await
            .map_err(serr)
    }

    async fn create(&self, path: &str, mode: u32) -> VfsResult<BlobFile<S>> {
        self.store
            .put_blob_async(&Self::data_key(path), &[])
            .await
            .map_err(serr)?;
        self.store_meta(path, &new_meta(path, 0, VfsFileType::Regular, mode))
            .await?;
        // A path created again is no longer whited out.
        self.store
            .delete_blob_async(&Self::whiteout_key(path))
            .await
            .map_err(serr)?;
        Ok(self.file(path, OpenMode::ReadWrite))
    }

    async fn mkdir(&self, path: &str) -> VfsResult<()> {
        self.store_meta(path, &new_meta(path, 0, VfsFileType::Directory, 0o755))
            .await?;
        self.store
            .delete_blob_async(&Self::whiteout_key(path))
            .await
            .map_err(serr)
    }

    async fn open(&self, path: &str, mode: OpenMode) -> VfsResult<BlobFile<S>> {
        if !self.exists(path).await? {
            return Err(not_found(path));
        }
        Ok(self.file(path, mode))
    }

    async fn open_directory(&self, path: &str) -> VfsResult<BlobDirectory<S>> {
        let meta = self.stat(path).await?;
        if meta.file_type != VfsFileType::Directory {
            return Err(ErrorTrace::new(VfsError::NotADirectory {
                path: path.to_string(),
            }));
        }
        Ok(BlobDirectory {
            path: path.to_string(),
            delta: self.clone(),
        })
    }

    async fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let meta = self.get_meta(from).await?;
        let data = self.read_data(from).await?;
        if meta.is_none() && data.is_none() {
            return Err(not_found(from));
        }
        if let Some(data) = data {
            self.store
                .put_blob_async(&Self::data_key(to), &data)
                .await
                .map_err(serr)?;
        }
        if let Some(meta) = meta {
            self.store_meta(to, &meta).await?;
        }
        self.store
            .delete_blob_async(&Self::data_key(from))
            .await
            .map_err(serr)?;
        self.store
            .delete_blob_async(&Self::meta_key(from))
            .await
            .map_err(serr)
    }

    async fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        self.store
            .put_blob_async(&Self::whiteout_key(path), &version.to_le_bytes())
            .await
            .map_err(serr)
    }

    fn file(&self, path: &str, mode: OpenMode) -> BlobFile<S> {
        BlobFile {
            path: path.to_string(),
            mode,
            pos: 0,
            delta: self.clone(),
        }
    }
}

/// A file in a [`BlobDelta`]. Reads and writes go to the store; the seekable
/// methods keep a cursor.
pub struct BlobFile<S: BlobStore + 'static> {
    path: String,
    mode: OpenMode,
    pos: u64,
    delta: BlobDelta<S>,
}

impl<S: BlobStore + 'static> BlobFile<S> {
    fn check_writable(&self) -> VfsResult<()> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        Ok(())
    }

    async fn content(&self) -> VfsResult<Vec<u8>> {
        Ok(self.delta.read_data(&self.path).await?.unwrap_or_default())
    }
}

#[async_trait]
impl<S: BlobStore + 'static> AsyncVfsFile for BlobFile<S> {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        let data = self.content().await?;
        let start = to_usize(offset)?;
        if start >= data.len() {
            return Ok(Vec::new());
        }
        let end = start.saturating_add(len).min(data.len());
        Ok(data[start..end].to_vec())
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        self.check_writable()?;
        let mut content = self.content().await?;
        let start = to_usize(offset)?;
        let end = start.saturating_add(data.len());
        if end > content.len() {
            content.resize(end, 0);
        }
        content[start..end].copy_from_slice(&data);
        self.delta.write_data(&self.path, &content).await?;
        Ok(data.len())
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        // Every write already reached the store.
        Ok(())
    }

    async fn size_async(&self) -> VfsResult<u64> {
        Ok(self.delta.stat(&self.path).await?.size)
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        self.check_writable()?;
        let mut content = self.content().await?;
        content.resize(to_usize(size)?, 0);
        self.delta.write_data(&self.path, &content).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.delta.stat(&self.path).await
    }
}

#[async_trait]
impl<S: BlobStore + 'static> AsyncSeekableVfsFile for BlobFile<S> {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let data = self.read_at_async(len, self.pos).await?;
        self.pos += data.len() as u64;
        Ok(data)
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        let n = self.write_at_async(data, self.pos).await?;
        self.pos += n as u64;
        Ok(n)
    }

    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let (base, delta) = match pos {
            SeekFrom::Start(p) => {
                self.pos = p;
                return Ok(p);
            }
            SeekFrom::End(d) => (self.size_async().await?, d),
            SeekFrom::Current(d) => (self.pos, d),
        };
        let new_pos = base.checked_add_signed(delta).ok_or_else(|| {
            ErrorTrace::new(VfsError::InvalidPath {
                path: format!("{}: seek before the start", self.path),
            })
        })?;
        self.pos = new_pos;
        Ok(new_pos)
    }

    fn position_async(&self) -> u64 {
        self.pos
    }
}

/// A directory in a [`BlobDelta`].
pub struct BlobDirectory<S: BlobStore + 'static> {
    path: String,
    delta: BlobDelta<S>,
}

#[async_trait]
impl<S: BlobStore + 'static> AsyncVfsDirectory for BlobDirectory<S> {
    type File = BlobFile<S>;
    type SeekableFile = BlobFile<S>;

    fn path(&self) -> String {
        self.path.clone()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.delta.stat(&self.path).await
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        // Blob stores can't list keys (backlog §13).
        Ok(Vec::new())
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        let path = join(&self.path, &name);
        if !self.delta.exists(&path).await? {
            return Ok(None);
        }
        let meta = self.delta.stat(&path).await?;
        Ok(Some(VfsDirEntry {
            inode: meta.inode,
            name,
            file_type: meta.file_type,
        }))
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> {
        self.delta.create(&join(&self.path, &name), mode).await
    }

    async fn create_dir_async(&self, name: String) -> VfsResult<BoxedDir<S>> {
        let path = join(&self.path, &name);
        self.delta.mkdir(&path).await?;
        Ok(Box::new(BlobDirectory {
            path,
            delta: self.delta.clone(),
        }))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        self.delta.add_whiteout(&join(&self.path, &name), 0).await
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        self.delta
            .rename(&join(&self.path, &old_name), &join(&self.path, &new_name))
            .await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        self.delta.open(&join(&self.path, &path), mode).await
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        self.delta.open(&join(&self.path, &path), mode).await
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<BoxedDir<S>> {
        let dir = if path == "." {
            self.path.clone()
        } else {
            join(&self.path, &path)
        };
        Ok(Box::new(self.delta.open_directory(&dir).await?))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.delta.stat(&join(&self.path, &path)).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.delta.exists(&join(&self.path, &path)).await
    }
}

#[async_trait]
impl<S: BlobStore + 'static> AsyncVfsFileSystem for BlobDelta<S> {
    type File = BlobFile<S>;
    type SeekableFile = BlobFile<S>;
    type Directory = BlobDirectory<S>;

    fn capabilities(&self) -> VfsCapabilities {
        self.capabilities
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.stat(&path).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.exists(&path).await
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        let mut meta = self.stat(&path).await?;
        meta.permissions = mode;
        self.store_meta(&path, &meta).await
    }

    async fn symlink_async(&self, _target: String, _link: String) -> VfsResult<()> {
        Err(unsupported("symlink"))
    }

    async fn readlink_async(&self, _path: String) -> VfsResult<String> {
        Err(unsupported("readlink"))
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        self.rename(&from, &to).await
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        self.add_whiteout(&path, 0).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        self.open(&path, mode).await
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        self.open(&path, mode).await
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        self.open_directory(&path).await
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        self.create(&path, mode).await
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        self.mkdir(&path).await
    }
}

#[async_trait]
impl<S: BlobStore + 'static> AsyncDeltaStore for BlobDelta<S> {
    async fn add_whiteout_async(&self, path: String, version: u64) -> VfsResult<()> {
        self.add_whiteout(&path, version).await
    }

    async fn is_whiteout_async(&self, path: String) -> VfsResult<Option<u64>> {
        self.whiteout(&path).await
    }

    async fn remove_whiteout_async(&self, path: String) -> VfsResult<()> {
        self.store
            .delete_blob_async(&Self::whiteout_key(&path))
            .await
            .map_err(serr)
    }

    async fn list_whiteouts_async(&self, _dir: String) -> VfsResult<Vec<(String, u64)>> {
        // Blob stores can't list keys (backlog §13).
        Ok(Vec::new())
    }

    async fn flush_async(&self) -> VfsResult<()> {
        // Every write already reached the store.
        Ok(())
    }

    async fn reset_async(&self) -> VfsResult<()> {
        Err(unsupported("reset (blob stores can't list keys to delete)"))
    }
}
