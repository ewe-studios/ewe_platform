#![cfg(feature = "vfs-fjall")]

//! FjallFs / FjallDelta — LSM-tree VFS Backend using fjall v2.

mod config;
mod content;
mod directory;
mod file_handle;
mod keyspaces;
mod path_index;
mod version;

pub use config::FjallVfsConfig;
pub use directory::FjallDirectory;
pub use file_handle::{FjallFile, SeekableFjallFile};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsMetadata};

use std::path::Path;
use std::sync::Arc;

/// FjallFs — complete VfsFileSystem backed by fjall.
pub struct FjallFs {
    keyspace: fjall::Keyspace,
    inodes: fjall::PartitionHandle,
    idx_path: fjall::PartitionHandle,
    config: FjallVfsConfig,
    root_ino: u64,
}

impl FjallFs {
    pub fn open(path: impl AsRef<Path>) -> VfsResult<Self> {
        Self::open_with_config(path, FjallVfsConfig::default())
    }

    pub fn open_with_config(path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)?;

        let keyspace = fjall::Config::new(path).open().map_err(|e| VfsError::Backend { message: e.to_string() })?;
        let inodes = keyspace.open_partition("inodes", fjall::PartitionCreateOptions::default()).map_err(|e| VfsError::Backend { message: e.to_string() })?;
        let idx_path = keyspace.open_partition("idx_path", fjall::PartitionCreateOptions::default()).map_err(|e| VfsError::Backend { message: e.to_string() })?;

        let root_ino = now_ms();

        // Bootstrap root directory
        keyspaces::write_inode(&inodes, root_ino, &keyspaces::FjallDentry {
            path: "/".to_string(),
            name: "/".to_string(),
            parent_ino: root_ino,
            file_type: crate::shared::vfs::types::VfsFileType::Directory,
            size: 0,
            permissions: 0o755,
            owner_uid: 0,
            owner_gid: 0,
            checksum: None,
            version: root_ino,
            created_at: root_ino,
            updated_at: root_ino,
            symlink_target: None,
            content_mode: content::ContentMode::Empty,
        })?;
        path_index::write_path_entries(&idx_path, root_ino, "/")?;

        Ok(Self { keyspace, inodes, idx_path, config, root_ino })
    }

    pub fn in_memory() -> VfsResult<Self> {
        let tmp = std::env::temp_dir().join(format!("fjallvfs-{}", std::process::id()));
        Self::open(&tmp)
    }

    fn clone_inner(&self) -> Arc<Self> {
        Arc::new(Self {
            keyspace: self.keyspace.clone(),
            inodes: self.inodes.clone(),
            idx_path: self.idx_path.clone(),
            config: self.config.clone(),
            root_ino: self.root_ino,
        })
    }
}

impl VfsFileSystem for FjallFs {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
    type Directory = FjallDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities { seekable: true, symlinks: true, permissions_enforced: false, event_emission: false, persistent: true }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let ino = path_index::resolve_path(&self.idx_path, path)?;
        let dentry = keyspaces::read_inode(&self.inodes, ino)?;
        Ok(version::dentry_to_metadata(&dentry))
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        path_index::resolve_path(&self.idx_path, path).map(|_| true)
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let ino = path_index::resolve_path(&self.idx_path, path)?;
        let mut dentry = keyspaces::read_inode(&self.inodes, ino)?;
        dentry.permissions = mode;
        dentry.updated_at = now_ms();
        keyspaces::write_inode(&self.inodes, ino, &dentry)
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let parent = link.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
        let parent_ino = path_index::resolve_path(&self.idx_path, parent).unwrap_or(self.root_ino);
        let name = link.rsplit_once('/').map(|(_, n)| n).unwrap_or(link).to_string();
        let ino = now_ms();
        let dentry = keyspaces::FjallDentry {
            path: link.to_string(), name, parent_ino,
            file_type: crate::shared::vfs::types::VfsFileType::Symlink,
            size: target.len() as u64, permissions: 0o777, owner_uid: 0, owner_gid: 0,
            checksum: None, version: ino,
            created_at: now_ms(), updated_at: now_ms(),
            symlink_target: Some(target.to_string()),
            content_mode: content::ContentMode::Empty,
        };
        keyspaces::write_inode(&self.inodes, ino, &dentry)?;
        path_index::write_path_entries(&self.idx_path, ino, link)
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let ino = path_index::resolve_path(&self.idx_path, path)?;
        let dentry = keyspaces::read_inode(&self.inodes, ino)?;
        dentry.symlink_target.ok_or_else(|| VfsError::Backend { message: "not a symlink".into() }.into())
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let ino = path_index::resolve_path(&self.idx_path, from)?;
        let mut dentry = keyspaces::read_inode(&self.inodes, ino)?;
        keyspaces::remove_path_entries(&self.idx_path, &dentry.path, ino)?;
        let parent = to.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
        dentry.parent_ino = path_index::resolve_path(&self.idx_path, parent).unwrap_or(self.root_ino);
        dentry.path = to.to_string();
        dentry.name = to.rsplit_once('/').map(|(_, n)| n).unwrap_or(to).to_string();
        dentry.updated_at = now_ms();
        keyspaces::write_inode(&self.inodes, ino, &dentry)?;
        path_index::write_path_entries(&self.idx_path, ino, to)
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let ino = path_index::resolve_path(&self.idx_path, path)?;
        let dentry = keyspaces::read_inode(&self.inodes, ino)?;
        if dentry.file_type == crate::shared::vfs::types::VfsFileType::Directory {
            let children = path_index::list_children(&self.idx_path, &self.inodes, path)?;
            if !children.is_empty() {
                return Err(VfsError::Backend { message: "directory not empty".into() }.into());
            }
        }
        keyspaces::remove_path_entries(&self.idx_path, &dentry.path, ino)?;
        keyspaces::delete_inode(&self.inodes, ino)
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let ino = path_index::resolve_path(&self.idx_path, path)?;
        let dentry = keyspaces::read_inode(&self.inodes, ino)?;
        Ok(FjallFile::new(ino, dentry, mode, self.clone_inner()))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableFjallFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let ino = path_index::resolve_path(&self.idx_path, path)?;
        let dentry = keyspaces::read_inode(&self.inodes, ino)?;
        if dentry.file_type != crate::shared::vfs::types::VfsFileType::Directory {
            return Err(VfsError::NotADirectory { path: path.to_string() }.into());
        }
        Ok(FjallDirectory::new(path.to_string(), self.clone_inner()))
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
        let parent_ino = path_index::resolve_path(&self.idx_path, parent).unwrap_or(self.root_ino);
        let name = path.rsplit_once('/').map(|(_, n)| n).unwrap_or(path).to_string();
        let ino = now_ms();
        let dentry = keyspaces::FjallDentry {
            path: path.to_string(), name, parent_ino,
            file_type: crate::shared::vfs::types::VfsFileType::Regular,
            size: 0, permissions: mode, owner_uid: 0, owner_gid: 0,
            checksum: None, version: ino,
            created_at: now_ms(), updated_at: now_ms(),
            symlink_target: None, content_mode: content::ContentMode::Empty,
        };
        keyspaces::write_inode(&self.inodes, ino, &dentry)?;
        path_index::write_path_entries(&self.idx_path, ino, path)?;
        Ok(FjallFile::new(ino, dentry, OpenMode::ReadWrite, self.clone_inner()))
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
        let parent_ino = path_index::resolve_path(&self.idx_path, parent).unwrap_or(self.root_ino);
        let name = path.rsplit_once('/').map(|(_, n)| n).unwrap_or(path).to_string();
        let ino = now_ms();
        let dentry = keyspaces::FjallDentry {
            path: path.to_string(), name, parent_ino,
            file_type: crate::shared::vfs::types::VfsFileType::Directory,
            size: 0, permissions: 0o755, owner_uid: 0, owner_gid: 0,
            checksum: None, version: ino,
            created_at: now_ms(), updated_at: now_ms(),
            symlink_target: None, content_mode: content::ContentMode::Empty,
        };
        keyspaces::write_inode(&self.inodes, ino, &dentry)?;
        path_index::write_path_entries(&self.idx_path, ino, path)
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let file = self.open(path, OpenMode::Read)?;
        let size = file.size()? as usize;
        let mut buf = vec![0u8; size];
        file.read_at(&mut buf, 0)?;
        Ok(buf)
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let file = self.open(path, OpenMode::Write)?;
        file.write_at(data, 0)?;
        Ok(())
    }

    fn copy(&self, from: &str, to: &str) -> VfsResult<()> {
        let data = self.read_file(from)?;
        let file = self.create(to, 0o644)?;
        file.write_at(&data, 0)?;
        Ok(())
    }

    fn remove_all(&self, path: &str) -> VfsResult<()> {
        let descendants = path_index::list_descendants(&self.idx_path, path)?;
        for ino in descendants.iter().rev() {
            keyspaces::remove_inode_and_paths(&self.inodes, &self.idx_path, ino)?;
        }
        if let Ok(ino) = path_index::resolve_path(&self.idx_path, path) {
            keyspaces::remove_inode_and_paths(&self.inodes, &self.idx_path, ino)?;
        }
        Ok(())
    }

    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').filter(|p| !p.is_empty()).collect();
        let mut current = String::new();
        for part in parts {
            current = format!("{current}/{part}");
            if self.exists(&current)? {
                let meta = self.stat(&current)?;
                if meta.file_type != crate::shared::vfs::types::VfsFileType::Directory {
                    return Err(VfsError::NotADirectory { path: current }.into());
                }
            } else {
                self.mkdir(&current)?;
            }
        }
        Ok(())
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        path_index::resolve_path(&self.idx_path, path)
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        keyspaces::read_inode(&self.inodes, ino).map(|d| d.path)
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        let dentry = keyspaces::read_inode(&self.inodes, ino)?;
        Ok(version::dentry_to_metadata(&dentry))
    }
}

// ── Async trait implementations ──
// fjall is synchronous — async wrappers delegate to sync methods

use crate::shared::vfs::async_traits::{
    AsyncVfsFile, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFileSystem, AsyncDeltaStore,
};
use async_trait::async_trait;

#[async_trait]
impl AsyncVfsFile for FjallFile {
    async fn read_at_async(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.read_at(buf, offset)
    }
    async fn write_at_async(&self, data: &[u8], offset: u64) -> VfsResult<usize> {
        self.write_at(data, offset)
    }
    async fn sync_data_async(&self) -> VfsResult<()> { self.sync_data() }
    async fn size_async(&self) -> VfsResult<u64> { self.size() }
    async fn truncate_async(&self, size: u64) -> VfsResult<()> { self.truncate(size) }
    async fn metadata_async(&self) -> VfsResult<VfsMetadata> { self.metadata() }
}

#[async_trait]
impl AsyncSeekableVfsFile for SeekableFjallFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let pos = *self.pos.read().unwrap();
        let mut buf = vec![0u8; len];
        let n = self.read_at(&mut buf, pos)?;
        *self.pos.write().unwrap() = pos + n as u64;
        buf.truncate(n);
        Ok(buf)
    }
    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        let pos = *self.pos.read().unwrap();
        let n = self.write_at(&data, pos)?;
        *self.pos.write().unwrap() = pos + n as u64;
        Ok(n)
    }
    async fn seek_async(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> { self.seek(pos) }
    fn position_async(&self) -> u64 { self.position() }
}

#[async_trait]
impl AsyncVfsDirectory for FjallDirectory {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
    fn path(&self) -> String { self.path.clone() }
    async fn metadata_async(&self) -> VfsResult<VfsMetadata> { self.metadata() }
    async fn list_async(&self) -> VfsResult<Vec<crate::shared::vfs::types::VfsDirEntry>> { self.list() }
    async fn get_entry_async(&self, name: String) -> VfsResult<Option<crate::shared::vfs::types::VfsDirEntry>> { self.get_entry(&name) }
    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> { self.create_file(&name, mode) }
    async fn create_dir_async(&self, name: String) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        self.create_dir(&name).map(|d| Box::new(*d) as Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
    }
    async fn remove_entry_async(&self, name: String) -> VfsResult<()> { self.remove_entry(&name) }
    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> { self.rename_entry(&old_name, &new_name) }
    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> { self.open(&path, mode) }
    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> { self.open_seekable(&path, mode) }
    async fn open_directory_async(&self, path: String) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        self.open_directory(&path).map(|d| Box::new(*d) as Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
    }
    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> { self.stat(&path) }
    async fn exists_async(&self, path: String) -> VfsResult<bool> { self.exists(&path) }
}

#[async_trait]
impl AsyncVfsFileSystem for FjallFs {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
    type Directory = FjallDirectory;

    fn capabilities(&self) -> VfsCapabilities { self.capabilities() }
    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> { self.stat(&path) }
    async fn exists_async(&self, path: String) -> VfsResult<bool> { self.exists(&path) }
    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> { self.chmod(&path, mode) }
    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> { self.symlink(&target, &link) }
    async fn readlink_async(&self, path: String) -> VfsResult<String> { self.readlink(&path) }
    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> { self.rename(&from, &to) }
    async fn remove_async(&self, path: String) -> VfsResult<()> { self.remove(&path) }
    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> { self.open(&path, mode) }
    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> { self.open_seekable(&path, mode) }
    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> { self.open_directory(&path) }
    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> { self.create(&path, mode) }
    async fn mkdir_async(&self, path: String) -> VfsResult<()> { self.mkdir(&path) }
}

#[async_trait]
impl AsyncVfsFileSystem for FjallDelta {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
    type Directory = FjallDirectory;

    fn capabilities(&self) -> VfsCapabilities { self.inner.capabilities() }
    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> { self.inner.stat(&path) }
    async fn exists_async(&self, path: String) -> VfsResult<bool> { self.exists(&path) }
    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> { self.inner.chmod(&path, mode) }
    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> { self.inner.symlink(&target, &link) }
    async fn readlink_async(&self, path: String) -> VfsResult<String> { self.inner.readlink(&path) }
    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> { self.inner.rename(&from, &to) }
    async fn remove_async(&self, path: String) -> VfsResult<()> { self.inner.remove(&path) }
    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> { self.inner.open(&path, mode) }
    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> { self.inner.open_seekable(&path, mode) }
    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> { self.inner.open_directory(&path) }
    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> { self.inner.create(&path, mode) }
    async fn mkdir_async(&self, path: String) -> VfsResult<()> { self.inner.mkdir(&path) }
}

#[async_trait]
impl AsyncDeltaStore for FjallDelta {
    async fn add_whiteout_async(&self, path: String, version: u64) -> VfsResult<()> { self.add_whiteout(&path, version) }
    async fn is_whiteout_async(&self, path: String) -> VfsResult<Option<u64>> { self.is_whiteout(&path) }
    async fn remove_whiteout_async(&self, path: String) -> VfsResult<()> { self.remove_whiteout(&path) }
    async fn list_whiteouts_async(&self, dir: String) -> VfsResult<Vec<(String, u64)>> { self.list_whiteouts(&dir) }
    async fn flush_async(&self) -> VfsResult<()> { self.flush() }
    async fn reset_async(&self) -> VfsResult<()> { self.reset() }
}

/// FjallDelta — DeltaStore extension of FjallFs with whiteout support.
pub struct FjallDelta {
    inner: FjallFs,
    whiteouts: fjall::PartitionHandle,
}

impl FjallDelta {
    pub fn open(path: impl AsRef<Path>) -> VfsResult<Self> {
        Self::open_with_config(path, FjallVfsConfig::default())
    }

    pub fn open_with_config(path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> {
        let inner = FjallFs::open_with_config(&path, config.clone())?;
        let whiteouts = inner.keyspace.open_partition("whiteouts", fjall::PartitionCreateOptions::default())
            .map_err(|e| VfsError::Backend { message: e.to_string() })?;
        Ok(Self { inner, whiteouts })
    }

    pub fn in_memory() -> VfsResult<Self> {
        let tmp = std::env::temp_dir().join(format!("fjallvfs-delta-{}", std::process::id()));
        Self::open(&tmp)
    }
}

impl VfsFileSystem for FjallDelta {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
    type Directory = FjallDirectory;

    fn capabilities(&self) -> VfsCapabilities { self.inner.capabilities() }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { self.inner.stat(path) }
    fn exists(&self, path: &str) -> VfsResult<bool> {
        if self.is_whiteout(path)?.is_some() { return Ok(false); }
        self.inner.exists(path)
    }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { self.inner.chmod(path, mode) }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { self.inner.symlink(target, link) }
    fn readlink(&self, path: &str) -> VfsResult<String> { self.inner.readlink(path) }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> { self.inner.rename(from, to) }
    fn remove(&self, path: &str) -> VfsResult<()> { self.inner.remove(path) }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { self.inner.open(path, mode) }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { self.inner.open_seekable(path, mode) }
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> { self.inner.open_directory(path) }
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> { self.inner.create(path, mode) }
    fn mkdir(&self, path: &str) -> VfsResult<()> { self.inner.mkdir(path) }
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> { self.inner.read_file(path) }
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> { self.inner.write_file(path, data) }
    fn copy(&self, from: &str, to: &str) -> VfsResult<()> { self.inner.copy(from, to) }
    fn remove_all(&self, path: &str) -> VfsResult<()> { self.inner.remove_all(path) }
    fn mkdir_all(&self, path: &str) -> VfsResult<()> { self.inner.mkdir_all(path) }
    fn inode(&self, path: &str) -> VfsResult<u64> { self.inner.inode(path) }
    fn path_by_inode(&self, ino: u64) -> VfsResult<String> { self.inner.path_by_inode(ino) }
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> { self.inner.stat_by_inode(ino) }
}

impl DeltaStore for FjallDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        path_index::write_whiteout_entries(&self.whiteouts, path, version)
    }
    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        path_index::is_whiteout(&self.whiteouts, path)
    }
    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        path_index::remove_whiteout_entries(&self.whiteouts, path)
    }
    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        path_index::list_whiteouts(&self.whiteouts, dir)
    }
    fn flush(&self) -> VfsResult<()> {
        self.inner.keyspace.persist(fjall::PersistMode::SyncAll).map_err(|e| VfsError::Backend { message: e.to_string() }.into())
    }
    fn reset(&self) -> VfsResult<()> {
        self.inner.inodes.clear();
        self.inner.idx_path.clear();
        self.whiteouts.clear();
        keyspaces::write_inode(&self.inner.inodes, self.inner.root_ino, &keyspaces::FjallDentry {
            path: "/".to_string(), name: "/".to_string(),
            parent_ino: self.inner.root_ino,
            file_type: crate::shared::vfs::types::VfsFileType::Directory,
            size: 0, permissions: 0o755, owner_uid: 0, owner_gid: 0,
            checksum: None, version: self.inner.root_ino,
            created_at: now_ms(), updated_at: now_ms(),
            symlink_target: None, content_mode: content::ContentMode::Empty,
        })?;
        path_index::write_path_entries(&self.inner.idx_path, self.inner.root_ino, "/")?;
        Ok(())
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
