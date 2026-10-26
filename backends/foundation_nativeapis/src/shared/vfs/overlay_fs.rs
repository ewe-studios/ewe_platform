//! `OverlayFileSystem`: a read-only base layer with a writable delta layer on
//! top, behind the async VFS traits.
//!
//! Reads go to the delta when it has the entry, else to the base. Writes go
//! to the delta; a base file is copied to the delta first (copy on write). A
//! removed base entry gets a whiteout in the delta. Sync consumers wrap the
//! overlay in `SyncFs`.

use std::collections::HashSet;
use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use foundation_errstacks::ErrorTrace;

use super::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use super::error::{VfsError, VfsResult};
use super::path_utils::{normalize_vfs_path, parent_path};
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsFileType, VfsMetadata};

/// The layers and the version counter, shared by the file system and the
/// directory handles it opens. A directory handle keeps the layers alive, so
/// it stays valid after the file system value is dropped.
struct OverlayCore<B, D> {
    base: Arc<B>,
    delta: Arc<D>,
    version: AtomicU64,
}

pub struct OverlayFileSystem<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> {
    core: Arc<OverlayCore<B, D>>,
}

impl<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> OverlayFileSystem<B, D> {
    pub fn new(base: B, delta: D) -> Self {
        Self::from_arcs(Arc::new(base), Arc::new(delta))
    }

    /// Builds the overlay over layers that other code also holds.
    pub fn from_arcs(base: Arc<B>, delta: Arc<D>) -> Self {
        Self {
            core: Arc::new(OverlayCore {
                base,
                delta,
                version: AtomicU64::new(0),
            }),
        }
    }

    pub fn base(&self) -> &B {
        &self.core.base
    }

    pub fn delta(&self) -> &D {
        &self.core.delta
    }

    pub fn version(&self) -> u64 {
        self.core.version.load(Ordering::SeqCst)
    }
}

impl<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> OverlayCore<B, D> {
    fn next_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    async fn is_visible(&self, path: &str) -> VfsResult<bool> {
        // A delta entry at the exact path always wins: the overlay creates
        // delta entries only after it handles whiteouts, so a delta entry
        // means "created after any deletion that made the whiteout".
        if self.delta.exists_async(path.to_string()).await? {
            return Ok(true);
        }
        if self
            .delta
            .is_whiteout_async(path.to_string())
            .await?
            .is_some()
        {
            return Ok(false);
        }
        self.base.exists_async(path.to_string()).await
    }

    async fn require_visible(&self, path: &str) -> VfsResult<()> {
        if self.is_visible(path).await? {
            Ok(())
        } else {
            Err(ErrorTrace::new(VfsError::NotFound {
                path: path.to_string(),
            }))
        }
    }

    async fn require_absent(&self, path: &str) -> VfsResult<()> {
        if self.is_visible(path).await? {
            Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: path.to_string(),
            }))
        } else {
            Ok(())
        }
    }

    async fn in_delta(&self, path: &str) -> VfsResult<bool> {
        self.delta.exists_async(path.to_string()).await
    }

    async fn ensure_delta_parents(&self, path: &str) -> VfsResult<()> {
        let mut current = String::new();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            current = format!("{current}/{part}");
            if !self.in_delta(&current).await? {
                self.delta.mkdir_async(current.clone()).await?;
            }
        }
        Ok(())
    }

    async fn ensure_parents_of(&self, path: &str) -> VfsResult<()> {
        match parent_path(path) {
            Some(parent) => self.ensure_delta_parents(&parent).await,
            None => Ok(()),
        }
    }

    /// Copies a base file to the same path in the delta.
    async fn copy_up_to(&self, from: &str, to: &str) -> VfsResult<()> {
        let data = self.base.read_file_async(from.to_string()).await?;
        let base_meta = self.base.stat_async(from.to_string()).await?;
        self.ensure_parents_of(to).await?;
        let file = self
            .delta
            .create_async(to.to_string(), base_meta.permissions)
            .await?;
        if let Err(e) = file.write_at_async(data, 0).await {
            // Remove the partial copy; the write error is the one to report.
            let _ = self.delta.remove_async(to.to_string()).await;
            return Err(e);
        }
        Ok(())
    }

    async fn stat(&self, path: String) -> VfsResult<VfsMetadata> {
        self.require_visible(&path).await?;
        if self.in_delta(&path).await? {
            self.delta.stat_async(path).await
        } else {
            self.base.stat_async(path).await
        }
    }

    async fn readlink(&self, path: String) -> VfsResult<String> {
        if self.in_delta(&path).await? {
            self.delta.readlink_async(path).await
        } else if self.base.exists_async(path.clone()).await? {
            self.base.readlink_async(path).await
        } else {
            Err(ErrorTrace::new(VfsError::NotFound { path }))
        }
    }

    async fn chmod(&self, path: String, mode: u32) -> VfsResult<()> {
        self.require_visible(&path).await?;
        if !self.in_delta(&path).await? {
            let meta = self.base.stat_async(path.clone()).await?;
            if meta.file_type == VfsFileType::Directory {
                self.ensure_delta_parents(&path).await?;
            } else {
                self.copy_up_to(&path, &path).await?;
            }
        }
        self.delta.chmod_async(path, mode).await
    }

    async fn symlink(&self, target: String, link: String) -> VfsResult<()> {
        self.require_absent(&link).await?;
        self.ensure_parents_of(&link).await?;
        self.delta.symlink_async(target, link).await
    }

    async fn rename(&self, from: String, to: String) -> VfsResult<()> {
        self.require_visible(&from).await?;
        self.require_absent(&to).await?;
        let in_delta = self.in_delta(&from).await?;
        let in_base = self.base.exists_async(from.clone()).await?;

        self.ensure_parents_of(&to).await?;
        if in_delta {
            self.delta.rename_async(from.clone(), to).await?;
        } else if in_base {
            self.copy_up_to(&from, &to).await?;
        }
        if in_base {
            let ver = self.next_version();
            self.delta.add_whiteout_async(from, ver).await?;
        }
        Ok(())
    }

    async fn remove(&self, path: String) -> VfsResult<()> {
        self.require_visible(&path).await?;
        let in_delta = self.in_delta(&path).await?;
        let in_base = self.base.exists_async(path.clone()).await?;
        if in_delta {
            self.delta.remove_async(path.clone()).await?;
        }
        if in_base {
            let ver = self.next_version();
            self.delta.add_whiteout_async(path, ver).await?;
        }
        Ok(())
    }

    /// Makes sure a writable open finds the file in the delta.
    async fn prepare_open(&self, path: &str, mode: OpenMode) -> VfsResult<bool> {
        self.require_visible(path).await?;
        let in_delta = self.in_delta(path).await?;
        if mode == OpenMode::Read {
            return Ok(in_delta);
        }
        if !in_delta {
            self.copy_up_to(path, path).await?;
        }
        Ok(true)
    }

    async fn open(&self, path: String, mode: OpenMode) -> VfsResult<OverlayFile<B::File, D::File>> {
        if self.prepare_open(&path, mode).await? {
            Ok(OverlayFile::Delta(self.delta.open_async(path, mode).await?))
        } else {
            Ok(OverlayFile::Base(self.base.open_async(path, mode).await?))
        }
    }

    async fn open_seekable(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<OverlaySeekableFile<B::SeekableFile, D::SeekableFile>> {
        if self.prepare_open(&path, mode).await? {
            Ok(OverlaySeekableFile::Delta(
                self.delta.open_seekable_async(path, mode).await?,
            ))
        } else {
            Ok(OverlaySeekableFile::Base(
                self.base.open_seekable_async(path, mode).await?,
            ))
        }
    }

    async fn create(&self, path: String, mode: u32) -> VfsResult<OverlayFile<B::File, D::File>> {
        self.require_absent(&path).await?;
        self.ensure_parents_of(&path).await?;
        self.next_version();
        Ok(OverlayFile::Delta(
            self.delta.create_async(path, mode).await?,
        ))
    }

    async fn mkdir(&self, path: String) -> VfsResult<()> {
        self.require_absent(&path).await?;
        self.ensure_parents_of(&path).await?;
        self.next_version();
        self.delta.mkdir_async(path).await
    }

    async fn read_file(&self, path: String) -> VfsResult<Vec<u8>> {
        self.require_visible(&path).await?;
        if self.in_delta(&path).await? {
            self.delta.read_file_async(path).await
        } else {
            self.base.read_file_async(path).await
        }
    }

    async fn write_file(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        self.next_version();
        if !self.in_delta(&path).await? {
            if self.base.exists_async(path.clone()).await? {
                self.copy_up_to(&path, &path).await?;
            } else {
                self.ensure_parents_of(&path).await?;
            }
        }
        self.delta.write_file_async(path, data).await
    }

    /// Lists a directory: delta entries first, then base entries that are
    /// neither shadowed by the delta nor hidden by a whiteout.
    async fn list(&self, dir_path: &str) -> VfsResult<Vec<VfsDirEntry>> {
        let mut entries = Vec::new();
        let mut seen_names = HashSet::new();

        if self.in_delta(dir_path).await? {
            let delta_dir = self
                .delta
                .open_directory_async(dir_path.to_string())
                .await?;
            for entry in delta_dir.list_async().await? {
                seen_names.insert(entry.name.clone());
                entries.push(entry);
            }
        }

        if self.base.exists_async(dir_path.to_string()).await? {
            let base_dir = self.base.open_directory_async(dir_path.to_string()).await?;
            for entry in base_dir.list_async().await? {
                if seen_names.contains(&entry.name) {
                    continue;
                }
                let child_path = join_child(dir_path, &entry.name);
                if self.delta.is_whiteout_async(child_path).await?.is_some() {
                    continue;
                }
                entries.push(entry);
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
}

fn join_child(dir: &str, name: &str) -> String {
    if dir == "/" {
        format!("/{name}")
    } else {
        format!("{dir}/{name}")
    }
}

// ── File handle types ──

pub enum OverlayFile<BF: AsyncVfsFile, DF: AsyncVfsFile> {
    Base(BF),
    Delta(DF),
}

#[async_trait]
impl<BF: AsyncVfsFile, DF: AsyncVfsFile> AsyncVfsFile for OverlayFile<BF, DF> {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        match self {
            OverlayFile::Base(f) => f.read_at_async(len, offset).await,
            OverlayFile::Delta(f) => f.read_at_async(len, offset).await,
        }
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        match self {
            OverlayFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlayFile::Delta(f) => f.write_at_async(data, offset).await,
        }
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        match self {
            OverlayFile::Base(f) => f.sync_data_async().await,
            OverlayFile::Delta(f) => f.sync_data_async().await,
        }
    }

    async fn size_async(&self) -> VfsResult<u64> {
        match self {
            OverlayFile::Base(f) => f.size_async().await,
            OverlayFile::Delta(f) => f.size_async().await,
        }
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        match self {
            OverlayFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlayFile::Delta(f) => f.truncate_async(size).await,
        }
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        match self {
            OverlayFile::Base(f) => f.metadata_async().await,
            OverlayFile::Delta(f) => f.metadata_async().await,
        }
    }
}

pub enum OverlaySeekableFile<BF: AsyncSeekableVfsFile, DF: AsyncSeekableVfsFile> {
    Base(BF),
    Delta(DF),
}

#[async_trait]
impl<BF: AsyncSeekableVfsFile, DF: AsyncSeekableVfsFile> AsyncVfsFile
    for OverlaySeekableFile<BF, DF>
{
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        match self {
            OverlaySeekableFile::Base(f) => f.read_at_async(len, offset).await,
            OverlaySeekableFile::Delta(f) => f.read_at_async(len, offset).await,
        }
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        match self {
            OverlaySeekableFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlaySeekableFile::Delta(f) => f.write_at_async(data, offset).await,
        }
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        match self {
            OverlaySeekableFile::Base(f) => f.sync_data_async().await,
            OverlaySeekableFile::Delta(f) => f.sync_data_async().await,
        }
    }

    async fn size_async(&self) -> VfsResult<u64> {
        match self {
            OverlaySeekableFile::Base(f) => f.size_async().await,
            OverlaySeekableFile::Delta(f) => f.size_async().await,
        }
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        match self {
            OverlaySeekableFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlaySeekableFile::Delta(f) => f.truncate_async(size).await,
        }
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        match self {
            OverlaySeekableFile::Base(f) => f.metadata_async().await,
            OverlaySeekableFile::Delta(f) => f.metadata_async().await,
        }
    }
}

#[async_trait]
impl<BF: AsyncSeekableVfsFile, DF: AsyncSeekableVfsFile> AsyncSeekableVfsFile
    for OverlaySeekableFile<BF, DF>
{
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        match self {
            OverlaySeekableFile::Base(f) => f.read_async(len).await,
            OverlaySeekableFile::Delta(f) => f.read_async(len).await,
        }
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        match self {
            OverlaySeekableFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlaySeekableFile::Delta(f) => f.write_async(data).await,
        }
    }

    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        match self {
            OverlaySeekableFile::Base(f) => f.seek_async(pos).await,
            OverlaySeekableFile::Delta(f) => f.seek_async(pos).await,
        }
    }

    fn position_async(&self) -> u64 {
        match self {
            OverlaySeekableFile::Base(f) => f.position_async(),
            OverlaySeekableFile::Delta(f) => f.position_async(),
        }
    }
}

// ── Overlay directory ──

pub struct OverlayDirectory<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> {
    core: Arc<OverlayCore<B, D>>,
    dir_path: String,
}

impl<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> OverlayDirectory<B, D> {
    fn resolve_child_path(&self, name: &str) -> VfsResult<String> {
        if name == "." || name.is_empty() {
            return Ok(self.dir_path.clone());
        }
        if name.starts_with('/') {
            return normalize_vfs_path(name);
        }
        normalize_vfs_path(&join_child(&self.dir_path, name))
    }

    fn child_dir(&self, dir_path: String) -> OverlayDirBox<B, D> {
        Box::new(OverlayDirectory {
            core: self.core.clone(),
            dir_path,
        })
    }
}

type OverlayDirBox<B, D> = Box<
    dyn AsyncVfsDirectory<
        File = OverlayFile<<B as AsyncVfsFileSystem>::File, <D as AsyncVfsFileSystem>::File>,
        SeekableFile = OverlaySeekableFile<
            <B as AsyncVfsFileSystem>::SeekableFile,
            <D as AsyncVfsFileSystem>::SeekableFile,
        >,
    >,
>;

#[async_trait]
impl<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> AsyncVfsDirectory
    for OverlayDirectory<B, D>
{
    type File = OverlayFile<B::File, D::File>;
    type SeekableFile = OverlaySeekableFile<B::SeekableFile, D::SeekableFile>;

    fn path(&self) -> String {
        self.dir_path.clone()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.core.stat(self.dir_path.clone()).await
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        self.core.list(&self.dir_path).await
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        let child_path = self.resolve_child_path(&name)?;
        if !self.core.is_visible(&child_path).await? {
            return Ok(None);
        }
        let meta = self.core.stat(child_path).await?;
        Ok(Some(VfsDirEntry {
            inode: meta.inode,
            name,
            file_type: meta.file_type,
        }))
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> {
        let child_path = self.resolve_child_path(&name)?;
        self.core.create(child_path, mode).await
    }

    async fn create_dir_async(&self, name: String) -> VfsResult<OverlayDirBox<B, D>> {
        let child_path = self.resolve_child_path(&name)?;
        self.core.mkdir(child_path.clone()).await?;
        Ok(self.child_dir(child_path))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        let child_path = self.resolve_child_path(&name)?;
        self.core.remove(child_path).await
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        let old_path = self.resolve_child_path(&old_name)?;
        let new_path = self.resolve_child_path(&new_name)?;
        self.core.rename(old_path, new_path).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        let full_path = self.resolve_child_path(&path)?;
        self.core.open(full_path, mode).await
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        let full_path = self.resolve_child_path(&path)?;
        self.core.open_seekable(full_path, mode).await
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<OverlayDirBox<B, D>> {
        let full_path = self.resolve_child_path(&path)?;
        self.core.require_visible(&full_path).await?;
        Ok(self.child_dir(full_path))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let full_path = self.resolve_child_path(&path)?;
        self.core.stat(full_path).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        let full_path = self.resolve_child_path(&path)?;
        self.core.is_visible(&full_path).await
    }
}

// ── AsyncVfsFileSystem for OverlayFileSystem ──

#[async_trait]
impl<B: AsyncVfsFileSystem + 'static, D: AsyncDeltaStore + 'static> AsyncVfsFileSystem
    for OverlayFileSystem<B, D>
{
    type File = OverlayFile<B::File, D::File>;
    type SeekableFile = OverlaySeekableFile<B::SeekableFile, D::SeekableFile>;
    type Directory = OverlayDirectory<B, D>;

    fn capabilities(&self) -> VfsCapabilities {
        let base_caps = self.core.base.capabilities();
        let delta_caps = self.core.delta.capabilities();
        VfsCapabilities {
            seekable: base_caps.seekable && delta_caps.seekable,
            symlinks: base_caps.symlinks && delta_caps.symlinks,
            permissions_enforced: false,
            event_emission: false,
            persistent: delta_caps.persistent,
        }
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.core.stat(normalize_vfs_path(&path)?).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.core.is_visible(&normalize_vfs_path(&path)?).await
    }

    async fn inode_async(&self, path: String) -> VfsResult<u64> {
        Ok(self.stat_async(path).await?.inode)
    }

    async fn path_by_inode_async(&self, ino: u64) -> VfsResult<String> {
        match self.core.delta.path_by_inode_async(ino).await {
            Ok(path) => Ok(path),
            Err(_) => self.core.base.path_by_inode_async(ino).await,
        }
    }

    async fn stat_by_inode_async(&self, ino: u64) -> VfsResult<VfsMetadata> {
        let path = self.path_by_inode_async(ino).await?;
        self.core.stat(path).await
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        self.core.chmod(normalize_vfs_path(&path)?, mode).await
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        self.core.symlink(target, normalize_vfs_path(&link)?).await
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        self.core.readlink(normalize_vfs_path(&path)?).await
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        self.core
            .rename(normalize_vfs_path(&from)?, normalize_vfs_path(&to)?)
            .await
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        self.core.remove(normalize_vfs_path(&path)?).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        self.core.open(normalize_vfs_path(&path)?, mode).await
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        self.core
            .open_seekable(normalize_vfs_path(&path)?, mode)
            .await
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        let path = normalize_vfs_path(&path)?;
        self.core.require_visible(&path).await?;
        Ok(OverlayDirectory {
            core: self.core.clone(),
            dir_path: path,
        })
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        self.core.create(normalize_vfs_path(&path)?, mode).await
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        self.core.mkdir(normalize_vfs_path(&path)?).await
    }

    async fn read_file_async(&self, path: String) -> VfsResult<Vec<u8>> {
        self.core.read_file(normalize_vfs_path(&path)?).await
    }

    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        self.core.write_file(normalize_vfs_path(&path)?, data).await
    }
}
