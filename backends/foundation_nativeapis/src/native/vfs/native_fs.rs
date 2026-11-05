//! `NativeFs`: the host file system under a root directory, behind the async
//! VFS traits.
//!
//! The standard library has no async file API. Bulk reads and writes above the
//! configured threshold run on a bounded offload pool so they do not hold a
//! valtron worker while a system call blocks. Small reads and writes, and the
//! metadata and control calls (`metadata`, `stat`, `size`, `sync_data`,
//! `truncate`), run inline: a local-file metadata call measures about 1 µs
//! (measured on macOS APFS and Linux ext4, warm cache), well under the 50 µs
//! budget. Sync consumers wrap `NativeFs` in `SyncFs`.

use std::io::{Read as IoRead, Seek as IoSeek, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use async_trait::async_trait;
use foundation_core::valtron::{BackgroundJobConfig, BackgroundJobRegistry, Pipe};
use foundation_errstacks::ErrorTrace;

use crate::shared::vfs::async_traits::{
    AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::path_utils::normalize_vfs_path as shared_normalize;
use crate::shared::vfs::types::{
    Checksum, OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType,
    VfsMetadata,
};

#[cfg(unix)]
use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};

/// Configuration for [`NativeFs`]'s offload pool (rule 8: limits are config,
/// not constants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFsConfig {
    threads: usize,
    queue_size: usize,
    threshold_bytes: usize,
}

impl Default for NativeFsConfig {
    fn default() -> Self {
        Self {
            threads: Self::DEFAULT_OFFLOAD_THREADS,
            queue_size: Self::DEFAULT_OFFLOAD_QUEUE_SIZE,
            threshold_bytes: Self::DEFAULT_OFFLOAD_THRESHOLD_BYTES,
        }
    }
}

impl NativeFsConfig {
    /// Default for [`Self::offload_threads`]: 2 worker threads. Two workers
    /// keep one blocked read from stalling the pool while a second operation
    /// proceeds; the pool is for blocking I/O, not CPU work.
    pub const DEFAULT_OFFLOAD_THREADS: usize = 2;

    /// Default for [`Self::offload_queue_size`]: 256 pending jobs. A submit
    /// past this bound returns an error instead of buffering without limit.
    /// (nginx keeps its accept queue at 511; 256 is enough for a local-root
    /// VFS where one caller rarely has more than a few hundred in flight.)
    pub const DEFAULT_OFFLOAD_QUEUE_SIZE: usize = 256;

    /// Default for [`Self::offload_threshold_bytes`]: 64 KiB. A read or write
    /// at or below this size completes in well under 50 µs on local storage
    /// (like the inline metadata calls), so it stays inline; a larger one
    /// moves to the offload pool. (Go's `net/http` copies buffers in 32 KiB
    /// chunks; 64 KiB is twice that chunk, still fast inline.)
    pub const DEFAULT_OFFLOAD_THRESHOLD_BYTES: usize = 64 * 1024;

    /// A config with the defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of offload worker threads.
    #[must_use]
    pub fn with_offload_threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    /// Bounded offload queue capacity. A submit past it returns an error.
    #[must_use]
    pub fn with_offload_queue_size(mut self, queue_size: usize) -> Self {
        self.queue_size = queue_size;
        self
    }

    /// Largest read or write, in bytes, that stays inline. Larger ones offload.
    #[must_use]
    pub fn with_offload_threshold_bytes(mut self, threshold: usize) -> Self {
        self.threshold_bytes = threshold;
        self
    }

    /// Number of offload worker threads.
    #[must_use]
    pub fn offload_threads(&self) -> usize {
        self.threads
    }

    /// Bounded offload queue capacity.
    #[must_use]
    pub fn offload_queue_size(&self) -> usize {
        self.queue_size
    }

    /// Largest read or write, in bytes, that stays inline.
    #[must_use]
    pub fn offload_threshold_bytes(&self) -> usize {
        self.threshold_bytes
    }
}

/// A bounded pool that runs blocking file I/O off the valtron workers.
///
/// Backed by valtron's [`BackgroundJobRegistry`]: a fixed worker count and a
/// bounded FIFO queue, with panic protection per job. Each offloaded job sends
/// its result back through a [`Pipe`], which the async caller awaits.
#[derive(Clone)]
struct OffloadPool {
    registry: Arc<BackgroundJobRegistry>,
    threshold: usize,
}

impl OffloadPool {
    fn new(config: &NativeFsConfig) -> VfsResult<Self> {
        if config.offload_threads() == 0 {
            return Err(ErrorTrace::new(VfsError::Backend {
                message: "NativeFs offload pool needs at least one worker thread".to_string(),
            }));
        }
        let background = BackgroundJobConfig::new(config.offload_threads())
            .with_capacity(config.offload_queue_size());
        let registry = BackgroundJobRegistry::with_config(background).map_err(|_| {
            ErrorTrace::new(VfsError::Backend {
                message: "failed to start the NativeFs offload pool".to_string(),
            })
        })?;
        Ok(Self {
            registry,
            threshold: config.offload_threshold_bytes(),
        })
    }

    /// `true` when an operation of `len` bytes should run on the offload pool.
    fn should_offload(&self, len: usize) -> bool {
        len > self.threshold
    }

    /// Run `job` on an offload worker and await its result.
    ///
    /// The submit returns an error when the bounded queue is full or closed;
    /// the caller then gets that error instead of unbounded buffering.
    async fn offload<T, F>(&self, job: F) -> VfsResult<T>
    where
        T: Send + 'static,
        F: FnOnce() -> VfsResult<T> + Send + 'static,
    {
        let (tx, rx) = Pipe::new();
        self.registry
            .submit(move || {
                let _ = tx.try_send(job());
            })
            .map_err(|err| {
                ErrorTrace::new(VfsError::Backend {
                    message: format!("NativeFs offload queue is full or closed: {err}"),
                })
            })?;
        match rx.receive().await {
            Some(result) => result,
            None => Err(ErrorTrace::new(VfsError::Backend {
                message: "NativeFs offload worker dropped the result channel".to_string(),
            })),
        }
    }
}

#[derive(Clone)]
pub struct NativeFs {
    root: Arc<PathBuf>,
    pool: OffloadPool,
}

impl NativeFs {
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    pub fn new(root: impl Into<PathBuf>) -> VfsResult<Self> {
        Self::with_config(root, NativeFsConfig::default())
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    pub fn with_config(root: impl Into<PathBuf>, config: NativeFsConfig) -> VfsResult<Self> {
        let root: PathBuf = root.into();
        let root = root
            .canonicalize()
            .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        if !root.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotADirectory {
                path: root.display().to_string(),
            }));
        }
        Ok(Self {
            root: Arc::new(root),
            pool: OffloadPool::new(&config)?,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve_path(&self, vfs_path: &str) -> VfsResult<PathBuf> {
        resolve_under(&self.root, vfs_path)
    }

    fn to_vfs_path(&self, fs_path: &Path) -> String {
        let relative = fs_path.strip_prefix(self.root.as_path()).unwrap_or(fs_path);
        let s = relative.to_string_lossy();
        if s.is_empty() {
            "/".to_string()
        } else {
            format!("/{s}")
        }
    }
}

/// Maps a VFS path to a host path under `root`.
///
/// A path with `..` is refused with [`VfsError::InvalidPath`], and a path
/// that leaves `root` through a symbolic link is refused with
/// [`VfsError::PermissionDenied`].
fn resolve_under(root: &Path, vfs_path: &str) -> VfsResult<PathBuf> {
    let vfs_path = shared_normalize(vfs_path)?;
    if vfs_path == "/" {
        return Ok(root.to_path_buf());
    }
    let relative = vfs_path.trim_start_matches('/');
    let joined = root.join(relative);

    // For existence-checking, we need to handle the case where the path
    // doesn't exist yet (create/mkdir). Try canonicalize; if it fails,
    // canonicalize the parent and append the last component.
    let resolved = if joined.exists() {
        joined.canonicalize().map_err(io_err)?
    } else {
        let parent = joined.parent().ok_or_else(|| {
            ErrorTrace::new(VfsError::InvalidPath {
                path: vfs_path.clone(),
            })
        })?;
        let name = joined.file_name().ok_or_else(|| {
            ErrorTrace::new(VfsError::InvalidPath {
                path: vfs_path.clone(),
            })
        })?;
        let resolved_parent = parent.canonicalize().map_err(io_err)?;
        if !resolved_parent.starts_with(root) {
            return Err(ErrorTrace::new(VfsError::PermissionDenied {
                path: format!("path escapes root: {vfs_path}"),
            }));
        }
        resolved_parent.join(name)
    };

    if !resolved.starts_with(root) {
        return Err(ErrorTrace::new(VfsError::PermissionDenied {
            path: format!("path escapes root: {vfs_path}"),
        }));
    }
    Ok(resolved)
}

fn io_err(e: std::io::Error) -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::Io { source: e })
}

fn open_host_file(fs_path: &Path, mode: OpenMode) -> VfsResult<std::fs::File> {
    match mode {
        OpenMode::Read => std::fs::File::open(fs_path),
        OpenMode::Write => std::fs::OpenOptions::new().write(true).open(fs_path),
        OpenMode::ReadWrite => std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(fs_path),
    }
    .map_err(io_err)
}

/// Checks that `fs_path` is an existing regular file (not a directory).
fn check_is_file(fs_path: &Path, vfs_path: &str) -> VfsResult<()> {
    if !fs_path.exists() {
        return Err(ErrorTrace::new(VfsError::NotFound {
            path: vfs_path.to_string(),
        }));
    }
    if fs_path.is_dir() {
        return Err(ErrorTrace::new(VfsError::NotAFile {
            path: vfs_path.to_string(),
        }));
    }
    Ok(())
}

fn create_host_file(
    fs_path: &Path,
    vfs_path: &str,
    mode: u32,
    pool: OffloadPool,
) -> VfsResult<NativeFile> {
    if fs_path.exists() {
        return Err(ErrorTrace::new(VfsError::AlreadyExists {
            path: vfs_path.to_string(),
        }));
    }
    // The handle is `ReadWrite`, so the host file is opened for both: a
    // write-only `File::create` handle failed every read.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(fs_path)
        .map_err(io_err)?;
    #[cfg(unix)]
    std::fs::set_permissions(fs_path, std::fs::Permissions::from_mode(mode)).map_err(io_err)?;
    #[cfg(not(unix))]
    let _ = mode;
    Ok(NativeFile {
        file,
        path: fs_path.to_path_buf(),
        mode: OpenMode::ReadWrite,
        pool,
    })
}

fn host_file_type(ft: std::fs::FileType) -> VfsFileType {
    if ft.is_dir() {
        VfsFileType::Directory
    } else if ft.is_symlink() {
        VfsFileType::Symlink
    } else {
        VfsFileType::Regular
    }
}

fn fs_metadata_to_vfs(meta: &std::fs::Metadata) -> VfsMetadata {
    let file_type = if meta.is_dir() {
        VfsFileType::Directory
    } else if meta.is_symlink() {
        VfsFileType::Symlink
    } else {
        VfsFileType::Regular
    };

    let permissions;
    let owner;
    #[cfg(unix)]
    {
        permissions = meta.mode();
        owner = (meta.uid(), meta.gid());
    }
    #[cfg(not(unix))]
    {
        permissions = if meta.permissions().readonly() {
            0o444
        } else {
            0o644
        };
        owner = (0, 0);
    }

    let inode;
    #[cfg(unix)]
    {
        inode = meta.ino();
    }
    #[cfg(not(unix))]
    {
        inode = 0;
    }

    VfsMetadata {
        inode,
        size: meta.len(),
        file_type,
        permissions,
        owner,
        created: meta.created().ok(),
        modified: meta.modified().ok(),
        accessed: meta.accessed().ok(),
        checksum: Checksum::None,
        version: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs()),
        state: VfsEntryState::Ready,
    }
}

// ── NativeFile ──

pub struct NativeFile {
    file: std::fs::File,
    path: PathBuf,
    mode: OpenMode,
    pool: OffloadPool,
}

impl std::fmt::Debug for NativeFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeFile")
            .field("file", &self.file)
            .field("path", &self.path)
            .field("mode", &self.mode)
            .field("pool", &format_args!("threshold={}", self.pool.threshold))
            .finish()
    }
}

/// Reads up to `len` bytes at `offset`; a short result means end of file.
#[cfg(unix)]
fn read_at_host(file: &std::fs::File, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let n = file.read_at(&mut buf, offset).map_err(io_err)?;
    buf.truncate(n);
    Ok(buf)
}

#[cfg(not(unix))]
fn read_at_host(_file: &std::fs::File, _len: usize, _offset: u64) -> VfsResult<Vec<u8>> {
    Err(ErrorTrace::new(VfsError::Unsupported {
        operation: "read_at on non-Unix".to_string(),
    }))
}

#[cfg(unix)]
fn write_at_host(file: &std::fs::File, data: &[u8], offset: u64) -> VfsResult<usize> {
    file.write_at(data, offset).map_err(io_err)
}

#[cfg(not(unix))]
fn write_at_host(_file: &std::fs::File, _data: &[u8], _offset: u64) -> VfsResult<usize> {
    Err(ErrorTrace::new(VfsError::Unsupported {
        operation: "write_at on non-Unix".to_string(),
    }))
}

#[async_trait]
impl AsyncVfsFile for NativeFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        if !self.pool.should_offload(len) {
            return read_at_host(&self.file, len, offset);
        }
        let file = self.file.try_clone().map_err(io_err)?;
        self.pool
            .offload(move || read_at_host(&file, len, offset))
            .await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        if !self.pool.should_offload(data.len()) {
            return write_at_host(&self.file, &data, offset);
        }
        let file = self.file.try_clone().map_err(io_err)?;
        self.pool
            .offload(move || write_at_host(&file, &data, offset))
            .await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        self.file.sync_data().map_err(io_err)
    }

    async fn size_async(&self) -> VfsResult<u64> {
        Ok(self.file.metadata().map_err(io_err)?.len())
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        self.file.set_len(size).map_err(io_err)
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        Ok(fs_metadata_to_vfs(&self.file.metadata().map_err(io_err)?))
    }
}

// ── SeekableNativeFile ──

pub struct SeekableNativeFile {
    file: RwLock<std::fs::File>,
    path: PathBuf,
    mode: OpenMode,
    position: AtomicU64,
    pool: OffloadPool,
}

impl std::fmt::Debug for SeekableNativeFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeekableNativeFile")
            .field("file", &self.file)
            .field("path", &self.path)
            .field("mode", &self.mode)
            .field("position", &self.position.load(Ordering::Acquire))
            .field("pool", &format_args!("threshold={}", self.pool.threshold))
            .finish()
    }
}

impl SeekableNativeFile {
    fn file(&self) -> VfsResult<std::sync::RwLockReadGuard<'_, std::fs::File>> {
        self.file.read().map_err(|_| {
            ErrorTrace::new(VfsError::Backend {
                message: "native file lock poisoned".into(),
            })
        })
    }

    fn file_mut(&self) -> VfsResult<std::sync::RwLockWriteGuard<'_, std::fs::File>> {
        self.file.write().map_err(|_| {
            ErrorTrace::new(VfsError::Backend {
                message: "native file lock poisoned".into(),
            })
        })
    }
}

#[async_trait]
impl AsyncVfsFile for SeekableNativeFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        if !self.pool.should_offload(len) {
            return read_at_host(&*self.file()?, len, offset);
        }
        let file = self.file()?.try_clone().map_err(io_err)?;
        self.pool
            .offload(move || read_at_host(&file, len, offset))
            .await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        if !self.pool.should_offload(data.len()) {
            return write_at_host(&*self.file()?, &data, offset);
        }
        let file = self.file()?.try_clone().map_err(io_err)?;
        self.pool
            .offload(move || write_at_host(&file, &data, offset))
            .await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        self.file()?.sync_data().map_err(io_err)
    }

    async fn size_async(&self) -> VfsResult<u64> {
        Ok(self.file()?.metadata().map_err(io_err)?.len())
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        self.file()?.set_len(size).map_err(io_err)
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        Ok(fs_metadata_to_vfs(
            &self.file()?.metadata().map_err(io_err)?,
        ))
    }
}

#[async_trait]
impl AsyncSeekableVfsFile for SeekableNativeFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let mut buf = vec![0u8; len];
        let n = IoRead::read(&mut *self.file_mut()?, &mut buf).map_err(io_err)?;
        buf.truncate(n);
        self.position.fetch_add(n as u64, Ordering::SeqCst);
        Ok(buf)
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let n = IoWrite::write(&mut *self.file_mut()?, &data).map_err(io_err)?;
        self.position.fetch_add(n as u64, Ordering::SeqCst);
        Ok(n)
    }

    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let new_pos = IoSeek::seek(&mut *self.file_mut()?, pos).map_err(io_err)?;
        self.position.store(new_pos, Ordering::SeqCst);
        Ok(new_pos)
    }

    fn position_async(&self) -> u64 {
        self.position.load(Ordering::SeqCst)
    }
}

// ── NativeDirectory ──

pub struct NativeDirectory {
    fs_root: Arc<PathBuf>,
    dir_path: String,
    pool: OffloadPool,
}

impl NativeDirectory {
    /// The VFS path of `name` in this directory (`"."` is the directory, an
    /// absolute name is used as is).
    fn vfs_path_of(&self, name: &str) -> String {
        if name == "." || name.is_empty() {
            self.dir_path.clone()
        } else if name.starts_with('/') {
            name.to_string()
        } else {
            self.child_vfs_path(name)
        }
    }

    /// Maps `name` to a host path. The same root checks as `NativeFs` apply,
    /// so a name with `..` cannot leave the root.
    fn resolve(&self, name: &str) -> VfsResult<PathBuf> {
        resolve_under(&self.fs_root, &self.vfs_path_of(name))
    }

    fn child_vfs_path(&self, name: &str) -> String {
        if self.dir_path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.dir_path)
        }
    }
}

type NativeDirBox =
    Box<dyn AsyncVfsDirectory<File = NativeFile, SeekableFile = SeekableNativeFile>>;

#[async_trait]
impl AsyncVfsDirectory for NativeDirectory {
    type File = NativeFile;
    type SeekableFile = SeekableNativeFile;

    fn path(&self) -> String {
        self.dir_path.clone()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        let fs_path = self.resolve(".")?;
        Ok(fs_metadata_to_vfs(
            &std::fs::metadata(&fs_path).map_err(io_err)?,
        ))
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let fs_path = self.resolve(".")?;
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&fs_path).map_err(io_err)? {
            let entry = entry.map_err(io_err)?;
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = host_file_type(entry.file_type().map_err(io_err)?);
            #[cfg(unix)]
            let inode = entry.metadata().map_err(io_err)?.ino();
            #[cfg(not(unix))]
            let inode = 0;
            entries.push(VfsDirEntry {
                inode,
                name,
                file_type,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        let fs_path = self.resolve(&name)?;
        if !fs_path.exists() {
            return Ok(None);
        }
        let meta = std::fs::symlink_metadata(&fs_path).map_err(io_err)?;
        #[cfg(unix)]
        let inode = meta.ino();
        #[cfg(not(unix))]
        let inode = 0;
        Ok(Some(VfsDirEntry {
            inode,
            file_type: host_file_type(meta.file_type()),
            name,
        }))
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<NativeFile> {
        let fs_path = self.resolve(&name)?;
        create_host_file(&fs_path, &self.vfs_path_of(&name), mode, self.pool.clone())
    }

    async fn create_dir_async(&self, name: String) -> VfsResult<NativeDirBox> {
        let fs_path = self.resolve(&name)?;
        let vfs_path = self.vfs_path_of(&name);
        if fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: vfs_path }));
        }
        std::fs::create_dir(&fs_path).map_err(io_err)?;
        Ok(Box::new(NativeDirectory {
            fs_root: self.fs_root.clone(),
            dir_path: vfs_path,
            pool: self.pool.clone(),
        }))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        let fs_path = self.resolve(&name)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: self.vfs_path_of(&name),
            }));
        }
        if fs_path.is_dir() {
            std::fs::remove_dir(&fs_path).map_err(io_err)
        } else {
            std::fs::remove_file(&fs_path).map_err(io_err)
        }
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        let old_path = self.resolve(&old_name)?;
        let new_path = self.resolve(&new_name)?;
        std::fs::rename(&old_path, &new_path).map_err(io_err)
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<NativeFile> {
        let fs_path = self.resolve(&path)?;
        check_is_file(&fs_path, &self.vfs_path_of(&path))?;
        Ok(NativeFile {
            file: open_host_file(&fs_path, mode)?,
            path: fs_path,
            mode,
            pool: self.pool.clone(),
        })
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<SeekableNativeFile> {
        let fs_path = self.resolve(&path)?;
        check_is_file(&fs_path, &self.vfs_path_of(&path))?;
        Ok(SeekableNativeFile {
            file: RwLock::new(open_host_file(&fs_path, mode)?),
            path: fs_path,
            mode,
            position: AtomicU64::new(0),
            pool: self.pool.clone(),
        })
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<NativeDirBox> {
        let fs_path = self.resolve(&path)?;
        let vfs_path = shared_normalize(&self.vfs_path_of(&path))?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound { path: vfs_path }));
        }
        if !fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotADirectory { path: vfs_path }));
        }
        Ok(Box::new(NativeDirectory {
            fs_root: self.fs_root.clone(),
            dir_path: vfs_path,
            pool: self.pool.clone(),
        }))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let fs_path = self.resolve(&path)?;
        Ok(fs_metadata_to_vfs(
            &std::fs::metadata(&fs_path).map_err(io_err)?,
        ))
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        Ok(self.resolve(&path).is_ok_and(|p| p.exists()))
    }
}

// ── AsyncVfsFileSystem for NativeFs ──

#[async_trait]
impl AsyncVfsFileSystem for NativeFs {
    type File = NativeFile;
    type SeekableFile = SeekableNativeFile;
    type Directory = NativeDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities {
            seekable: true,
            symlinks: true,
            permissions_enforced: cfg!(unix),
            event_emission: false,
            persistent: true,
        }
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let fs_path = self.resolve_path(&path)?;
        Ok(fs_metadata_to_vfs(
            &std::fs::metadata(&fs_path).map_err(io_err)?,
        ))
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        Ok(self.resolve_path(&path).is_ok_and(|p| p.exists()))
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        let fs_path = self.resolve_path(&path)?;
        #[cfg(unix)]
        {
            std::fs::set_permissions(&fs_path, std::fs::Permissions::from_mode(mode))
                .map_err(io_err)
        }
        #[cfg(not(unix))]
        {
            let _ = (fs_path, mode);
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "chmod on non-Unix".to_string(),
            }))
        }
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        let link_path = self.resolve_path(&link)?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, &link_path).map_err(io_err)
        }
        #[cfg(not(unix))]
        {
            let _ = (target, link_path);
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "symlink on non-Unix".to_string(),
            }))
        }
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        let fs_path = self.resolve_path(&path)?;
        let target = std::fs::read_link(&fs_path).map_err(io_err)?;
        Ok(target.to_string_lossy().to_string())
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        let from_path = self.resolve_path(&from)?;
        let to_path = self.resolve_path(&to)?;
        std::fs::rename(&from_path, &to_path).map_err(io_err)
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        let fs_path = self.resolve_path(&path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        if fs_path.is_dir() {
            std::fs::remove_dir(&fs_path).map_err(io_err)
        } else {
            std::fs::remove_file(&fs_path).map_err(io_err)
        }
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<NativeFile> {
        let fs_path = self.resolve_path(&path)?;
        check_is_file(&fs_path, &path)?;
        Ok(NativeFile {
            file: open_host_file(&fs_path, mode)?,
            path: fs_path,
            mode,
            pool: self.pool.clone(),
        })
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<SeekableNativeFile> {
        let fs_path = self.resolve_path(&path)?;
        check_is_file(&fs_path, &path)?;
        Ok(SeekableNativeFile {
            file: RwLock::new(open_host_file(&fs_path, mode)?),
            path: fs_path,
            mode,
            position: AtomicU64::new(0),
            pool: self.pool.clone(),
        })
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<NativeDirectory> {
        let fs_path = self.resolve_path(&path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        if !fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotADirectory { path }));
        }
        Ok(NativeDirectory {
            fs_root: self.root.clone(),
            dir_path: self.to_vfs_path(&fs_path),
            pool: self.pool.clone(),
        })
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<NativeFile> {
        let fs_path = self.resolve_path(&path)?;
        create_host_file(&fs_path, &path, mode, self.pool.clone())
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        let fs_path = self.resolve_path(&path)?;
        if fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path }));
        }
        std::fs::create_dir(&fs_path).map_err(io_err)
    }

    async fn read_file_async(&self, path: String) -> VfsResult<Vec<u8>> {
        let fs_path = self.resolve_path(&path)?;
        let size = std::fs::metadata(&fs_path)
            .map_err(io_err)?
            .len()
            .try_into()
            .unwrap_or(usize::MAX);
        if !self.pool.should_offload(size) {
            return std::fs::read(&fs_path).map_err(io_err);
        }
        self.pool
            .offload(move || std::fs::read(&fs_path).map_err(io_err))
            .await
    }

    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        let fs_path = self.resolve_path(&path)?;
        if !self.pool.should_offload(data.len()) {
            return std::fs::write(&fs_path, data).map_err(io_err);
        }
        self.pool
            .offload(move || std::fs::write(&fs_path, data).map_err(io_err))
            .await
    }
}
