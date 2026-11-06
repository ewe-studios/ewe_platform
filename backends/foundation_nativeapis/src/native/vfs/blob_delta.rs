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
//! - `vfs/data/{path}/{chunk}`: the file's content, split into fixed-size
//!   chunks;
//! - `vfs/meta/{path}`: its `VfsMetadata` as JSON;
//! - `vfs/whiteout/{path}`: a whiteout tombstone (the version, little-endian
//!   `u64`).
//!
//! HOW: a write splices the bytes in at the offset and writes back only the
//! chunks it touched (their bytes changed or their range appeared/disappeared).
//! Directory listing, whiteout listing and reset walk the store through
//! [`BlobStore::list_blobs_after`].

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::SeekFrom;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_core::valtron::Clock;
use foundation_db::{BlobStore, StorageError};
use foundation_errstacks::ErrorTrace;
use futures_lite::StreamExt;

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

/// Decode the little-endian `u64` version stored in a whiteout blob.
fn whiteout_version(bytes: &[u8]) -> u64 {
    bytes
        .get(..8)
        .and_then(|b| <[u8; 8]>::try_from(b).ok())
        .map_or(0, u64::from_le_bytes)
}

/// The bytes of the chunk that starts at `start` (at most `chunk_size` bytes,
/// fewer past the end of `content`, and none before it).
fn chunk_slice(content: &[u8], start: usize, chunk_size: usize) -> &[u8] {
    if start >= content.len() {
        return &[];
    }
    let end = (start + chunk_size).min(content.len());
    &content[start..end]
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

fn new_meta(
    path: &str,
    size: u64,
    file_type: VfsFileType,
    permissions: u32,
    clock: &dyn Clock,
) -> VfsMetadata {
    let now = clock.wall_now();
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

/// Configuration for [`BlobDelta`].
#[derive(Debug, Clone, Copy)]
pub struct BlobDeltaConfig {
    /// Bytes per stored chunk. A write rewrites only the chunks it touches, so
    /// this bounds the amount rewritten for a small write.
    ///
    /// Default: 64 KiB. Go's `net/http` copies buffers in 32 KiB chunks; 64 KiB
    /// is twice that chunk, which keeps a 4 KiB write to at most two chunks
    /// (one round trip) while a 1 MiB file stays at 16 chunks.
    pub chunk_size: usize,
}

impl Default for BlobDeltaConfig {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024,
        }
    }
}

impl BlobDeltaConfig {
    /// The default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the chunk size. A value of zero becomes one byte.
    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.max(1);
        self
    }
}

/// A delta store over an async blob store. See the module docs.
pub struct BlobDelta<S: BlobStore + 'static> {
    store: Arc<S>,
    capabilities: VfsCapabilities,
    chunk_size: usize,
    clock: Arc<dyn Clock>,
}

impl<S: BlobStore + 'static> Clone for BlobDelta<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            capabilities: self.capabilities,
            chunk_size: self.chunk_size,
            clock: Arc::clone(&self.clock),
        }
    }
}

impl<S: BlobStore + 'static> BlobDelta<S> {
    /// A delta over `store` with the default configuration.
    #[must_use]
    pub fn from_store(store: S, clock: Arc<dyn Clock>) -> Self {
        Self::from_store_with_config(store, BlobDeltaConfig::default(), clock)
    }

    /// A delta over `store` with `config`.
    #[must_use]
    pub fn from_store_with_config(
        store: S,
        config: BlobDeltaConfig,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store: Arc::new(store),
            capabilities: VfsCapabilities {
                seekable: true,
                symlinks: false,
                permissions_enforced: false,
                event_emission: false,
                persistent: true,
            },
            chunk_size: config.chunk_size.max(1),
            clock,
        }
    }

    /// The underlying store.
    #[must_use]
    pub fn store(&self) -> &S {
        &self.store
    }

    fn data_prefix(path: &str) -> String {
        format!("vfs/data/{}/", path.trim_start_matches('/'))
    }

    fn chunk_key(path: &str, index: u64) -> String {
        format!("vfs/data/{}/{index:016x}", path.trim_start_matches('/'))
    }

    fn meta_key(path: &str) -> String {
        format!("vfs/meta/{}", path.trim_start_matches('/'))
    }

    fn whiteout_key(path: &str) -> String {
        format!("vfs/whiteout/{}", path.trim_start_matches('/'))
    }

    /// The meta-key prefix for every direct child of `path`.
    fn meta_dir_prefix(path: &str) -> String {
        let trimmed = path.trim_start_matches('/');
        if trimmed.is_empty() {
            "vfs/meta/".to_string()
        } else if trimmed.ends_with('/') {
            format!("vfs/meta/{trimmed}")
        } else {
            format!("vfs/meta/{trimmed}/")
        }
    }

    /// The whiteout-key prefix for every path under `dir`.
    fn whiteout_dir_prefix(dir: &str) -> String {
        let trimmed = dir.trim_start_matches('/');
        if trimmed.is_empty() {
            "vfs/whiteout/".to_string()
        } else if trimmed.ends_with('/') {
            format!("vfs/whiteout/{trimmed}")
        } else {
            format!("vfs/whiteout/{trimmed}/")
        }
    }

    /// All blob keys that start with `prefix`, in sorted order.
    async fn list_keys_after(&self, prefix: &str) -> VfsResult<Vec<String>> {
        let stream = self
            .store
            .list_blobs_after(prefix, None, None)
            .await
            .map_err(serr)?;
        stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(serr)
    }

    async fn read_data(&self, path: &str) -> VfsResult<Option<Vec<u8>>> {
        let mut out = Vec::new();
        let mut index = 0u64;
        let mut found = false;
        loop {
            match self
                .store
                .get_blob(&Self::chunk_key(path, index))
                .await
                .map_err(serr)?
            {
                Some(bytes) => {
                    found = true;
                    out.extend_from_slice(&bytes);
                    index += 1;
                }
                None => break,
            }
        }
        Ok(found.then_some(out))
    }

    async fn get_meta(&self, path: &str) -> VfsResult<Option<VfsMetadata>> {
        match self
            .store
            .get_blob(&Self::meta_key(path))
            .await
            .map_err(serr)?
        {
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
            .put_blob(&Self::meta_key(path), &bytes)
            .await
            .map_err(serr)
    }

    /// Rewrite only the chunks whose bytes differ between `old_content` and
    /// `new_content`; delete chunks that `new_content` no longer spans.
    async fn replace_chunks(
        &self,
        path: &str,
        old_content: &[u8],
        new_content: &[u8],
    ) -> VfsResult<()> {
        let chunk_size = self.chunk_size.max(1);
        let old_chunks = old_content.len().div_ceil(chunk_size);
        let new_chunks = new_content.len().div_ceil(chunk_size);
        for index in 0..old_chunks.max(new_chunks) {
            let start = index * chunk_size;
            let old_slice = chunk_slice(old_content, start, chunk_size);
            let new_slice = chunk_slice(new_content, start, chunk_size);
            if old_slice == new_slice {
                continue;
            }
            let key = Self::chunk_key(path, index as u64);
            if new_slice.is_empty() {
                self.store.delete_blob(&key).await.map_err(serr)?;
            } else {
                self.store.put_blob(&key, new_slice).await.map_err(serr)?;
            }
        }
        Ok(())
    }

    /// Update the stored size and mtime for `path`, creating a regular-file
    /// metadata record if none exists.
    async fn touch_meta(&self, path: &str, size: u64) -> VfsResult<()> {
        let mut meta = self
            .get_meta(path)
            .await?
            .unwrap_or_else(|| new_meta(path, 0, VfsFileType::Regular, 0o644, self.clock.as_ref()));
        meta.size = size;
        meta.modified = Some(self.clock.wall_now());
        self.store_meta(path, &meta).await
    }

    async fn whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        match self
            .store
            .get_blob(&Self::whiteout_key(path))
            .await
            .map_err(serr)?
        {
            Some(bytes) => Ok(Some(whiteout_version(&bytes))),
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
            Some(data) => Ok(new_meta(
                path,
                data.len() as u64,
                VfsFileType::Regular,
                0o644,
                self.clock.as_ref(),
            )),
            None => Err(not_found(path)),
        }
    }

    async fn exists(&self, path: &str) -> VfsResult<bool> {
        if self.whiteout(path).await?.is_some() {
            return Ok(false);
        }
        if self
            .store
            .blob_exists(&Self::meta_key(path))
            .await
            .map_err(serr)?
        {
            return Ok(true);
        }
        self.store
            .blob_exists(&Self::chunk_key(path, 0))
            .await
            .map_err(serr)
    }

    async fn create(&self, path: &str, mode: u32) -> VfsResult<BlobFile<S>> {
        // A re-created path starts empty: drop any chunks it still has.
        for key in self.list_keys_after(&Self::data_prefix(path)).await? {
            self.store.delete_blob(&key).await.map_err(serr)?;
        }
        self.store_meta(
            path,
            &new_meta(path, 0, VfsFileType::Regular, mode, self.clock.as_ref()),
        )
        .await?;
        // A path created again is no longer whited out.
        self.store
            .delete_blob(&Self::whiteout_key(path))
            .await
            .map_err(serr)?;
        Ok(self.file(path, OpenMode::ReadWrite))
    }

    async fn mkdir(&self, path: &str) -> VfsResult<()> {
        self.store_meta(
            path,
            &new_meta(path, 0, VfsFileType::Directory, 0o755, self.clock.as_ref()),
        )
        .await?;
        self.store
            .delete_blob(&Self::whiteout_key(path))
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
            let to_old = self.read_data(to).await?.unwrap_or_default();
            self.replace_chunks(to, &to_old, &data).await?;
        }
        if let Some(meta) = meta {
            self.store_meta(to, &meta).await?;
        }
        for key in self.list_keys_after(&Self::data_prefix(from)).await? {
            self.store.delete_blob(&key).await.map_err(serr)?;
        }
        self.store
            .delete_blob(&Self::meta_key(from))
            .await
            .map_err(serr)?;
        self.store
            .delete_blob(&Self::whiteout_key(from))
            .await
            .map_err(serr)
    }

    async fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        self.store
            .put_blob(&Self::whiteout_key(path), &version.to_le_bytes())
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
        let old = self.content().await?;
        let start = to_usize(offset)?;
        let end = start.saturating_add(data.len());
        let mut content = old.clone();
        if end > content.len() {
            content.resize(end, 0);
        }
        content[start..end].copy_from_slice(&data);
        self.delta
            .replace_chunks(&self.path, &old, &content)
            .await?;
        self.delta
            .touch_meta(&self.path, content.len() as u64)
            .await?;
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
        let old = self.content().await?;
        let mut content = old.clone();
        content.resize(to_usize(size)?, 0);
        self.delta
            .replace_chunks(&self.path, &old, &content)
            .await?;
        self.delta
            .touch_meta(&self.path, content.len() as u64)
            .await
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
        let prefix = BlobDelta::<S>::meta_dir_prefix(&self.path);
        let mut entries = Vec::new();
        for key in self.delta.list_keys_after(&prefix).await? {
            let Some(name) = key.strip_prefix(&prefix) else {
                continue;
            };
            // Only direct children; nested names carry a `/`.
            if name.is_empty() || name.contains('/') {
                continue;
            }
            let child = join(&self.path, name);
            if let Ok(meta) = self.delta.stat(&child).await {
                entries.push(VfsDirEntry {
                    inode: meta.inode,
                    name: name.to_string(),
                    file_type: meta.file_type,
                });
            }
        }
        Ok(entries)
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

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
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

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
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
            .delete_blob(&Self::whiteout_key(&path))
            .await
            .map_err(serr)
    }

    async fn list_whiteouts_async(&self, dir: String) -> VfsResult<Vec<(String, u64)>> {
        let prefix = Self::whiteout_dir_prefix(&dir);
        let mut out = Vec::new();
        for key in self.list_keys_after(&prefix).await? {
            let Some(rel) = key.strip_prefix("vfs/whiteout/") else {
                continue;
            };
            let path = format!("/{rel}");
            let version = match self.store.get_blob(&key).await.map_err(serr)? {
                Some(bytes) => whiteout_version(&bytes),
                None => 0,
            };
            out.push((path, version));
        }
        Ok(out)
    }

    async fn flush_async(&self) -> VfsResult<()> {
        // Every write already reached the store.
        Ok(())
    }

    async fn reset_async(&self) -> VfsResult<()> {
        for key in self.list_keys_after("vfs/").await? {
            self.store.delete_blob(&key).await.map_err(serr)?;
        }
        Ok(())
    }
}
