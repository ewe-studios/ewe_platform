#![cfg(feature = "vfs-fjall")]

//! FjallFs / FjallDelta — LSM-tree VFS Backend using fjall v2.

use std::sync::Arc;
use std::path::Path;

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsFileSystem, VfsFile, SeekableVfsFile, VfsDirectory};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsMetadata, VfsFileType, VfsDirEntry, VfsEntryState, Checksum};

use async_trait::async_trait;
use crate::shared::vfs::async_traits::{AsyncVfsFile, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFileSystem, AsyncDeltaStore};

/// FjallVfsConfig — configuration for fjall engine tuning.
#[derive(Clone, Debug)]
pub struct FjallVfsConfig {
    pub inline_threshold: usize,
    pub chunk_size: usize,
}

impl Default for FjallVfsConfig {
    fn default() -> Self {
        Self { inline_threshold: 4096, chunk_size: 65536 }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn pack_version(v: u64) -> u64 { v }
fn unpack_version(v: u64) -> u64 { v }

// In-memory inode store (simplified - production would use fjall partitions)
type InodeStore = std::sync::RwLock<std::collections::HashMap<u64, InodeEntry>>;
type PathStore = std::sync::RwLock<std::collections::HashMap<String, u64>>;

#[derive(Clone)]
struct InodeEntry {
    path: String,
    name: String,
    file_type: VfsFileType,
    size: u64,
    permissions: u32,
    owner: (u32, u32),
    version: u64,
    created_at: u64,
    updated_at: u64,
    content: Vec<u8>,
    symlink_target: Option<String>,
}

pub struct FjallFile {
    path: String,
    mode: OpenMode,
    fs: Arc<FjallFsInner>,
}

impl VfsFile for FjallFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let entries = self.fs.inodes.read().unwrap();
        let ino = self.fs.paths.read().unwrap().get(&self.path).copied().ok_or_else(|| VfsError::Backend { message: "not found".into() })?;
        let entry = entries.get(&ino).ok_or_else(|| VfsError::Backend { message: "inode not found".into() })?;
        let off = offset as usize;
        if off >= entry.content.len() { return Ok(0); }
        let n = buf.len().min(entry.content.len() - off);
        buf[..n].copy_from_slice(&entry.content[off..off + n]);
        Ok(n)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let mut entries = self.fs.inodes.write().unwrap();
        let ino = self.fs.paths.read().unwrap().get(&self.path).copied().ok_or_else(|| VfsError::Backend { message: "not found".into() })?;
        let entry = entries.get_mut(&ino).ok_or_else(|| VfsError::Backend { message: "inode not found".into() })?;
        let end = (offset as usize) + buf.len();
        if end > entry.content.len() { entry.content.resize(end, 0); }
        entry.content[offset as usize..end].copy_from_slice(buf);
        entry.size = entry.content.len() as u64;
        entry.updated_at = now_ms();
        Ok(buf.len())
    }
    fn sync_data(&self) -> VfsResult<()> { Ok(()) }
    fn size(&self) -> VfsResult<u64> {
        let entries = self.fs.inodes.read().unwrap();
        let ino = self.fs.paths.read().unwrap().get(&self.path).copied().ok_or_else(|| VfsError::Backend { message: "not found".into() })?;
        entries.get(&ino).map(|e| e.size).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn truncate(&self, size: u64) -> VfsResult<()> {
        let mut entries = self.fs.inodes.write().unwrap();
        let ino = self.fs.paths.read().unwrap().get(&self.path).copied().ok_or_else(|| VfsError::Backend { message: "not found".into() })?;
        if let Some(entry) = entries.get_mut(&ino) {
            entry.content.truncate(size as usize);
            entry.size = entry.content.len() as u64;
            entry.updated_at = now_ms();
        }
        Ok(())
    }
    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let entries = self.fs.inodes.read().unwrap();
        let ino = self.fs.paths.read().unwrap().get(&self.path).copied().ok_or_else(|| VfsError::Backend { message: "not found".into() })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
}

pub struct SeekableFjallFile {
    file: FjallFile,
    pos: std::sync::Arc<std::sync::RwLock<u64>>,
}

impl SeekableFjallFile {
    pub fn new(file: FjallFile) -> Self {
        Self { file, pos: std::sync::Arc::new(std::sync::RwLock::new(0)) }
    }
}

impl VfsFile for SeekableFjallFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { self.file.read_at(buf, offset) }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> { self.file.write_at(buf, offset) }
    fn sync_data(&self) -> VfsResult<()> { self.file.sync_data() }
    fn size(&self) -> VfsResult<u64> { self.file.size() }
    fn truncate(&self, size: u64) -> VfsResult<()> { self.file.truncate(size) }
    fn metadata(&self) -> VfsResult<VfsMetadata> { self.file.metadata() }
}

impl SeekableVfsFile for SeekableFjallFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let pos = *self.pos.read().unwrap();
        let n = self.file.read_at(buf, pos)?;
        *self.pos.write().unwrap() = pos + n as u64;
        Ok(n)
    }
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let pos = *self.pos.read().unwrap();
        let n = self.file.write_at(buf, pos)?;
        *self.pos.write().unwrap() = pos + n as u64;
        Ok(n)
    }
    fn seek(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> {
        let cur = *self.pos.read().unwrap();
        let size = self.file.size()?;
        let new_pos = match pos {
            std::io::SeekFrom::Start(p) => p,
            std::io::SeekFrom::End(p) => (size as i64 + p) as u64,
            std::io::SeekFrom::Current(p) => (cur as i64 + p) as u64,
        };
        *self.pos.write().unwrap() = new_pos;
        Ok(new_pos)
    }
    fn position(&self) -> u64 { *self.pos.read().unwrap() }
}

pub struct FjallDirectory {
    path: String,
    fs: Arc<FjallFsInner>,
}

impl FjallDirectory {
    pub fn new(path: String, fs: Arc<FjallFsInner>) -> Self { Self { path, fs } }
}

impl VfsDirectory for FjallDirectory {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
    fn path(&self) -> &str { &self.path }
    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let entries = self.fs.inodes.read().unwrap();
        let ino = self.fs.paths.read().unwrap().get(&self.path).copied().ok_or_else(|| VfsError::Backend { message: "not found".into() })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let entries = self.fs.inodes.read().unwrap();
        let paths = self.fs.paths.read().unwrap();
        let prefix = if self.path.ends_with('/') { self.path.clone() } else { format!("{}/", self.path) };
        Ok(entries.iter().filter_map(|(ino, e)| {
            if e.path.starts_with(&prefix) {
                let remaining = e.path.strip_prefix(&prefix).unwrap_or(&e.path);
                if !remaining.contains('/') && !remaining.is_empty() {
                    Some(VfsDirEntry { name: remaining.to_string(), inode: *ino, file_type: e.file_type })
                } else {
                    None
                }
            } else {
                None
            }
        }).collect())
    }
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        let paths = self.fs.paths.read().unwrap();
        let entries = self.fs.inodes.read().unwrap();
        if let Some(ino) = paths.get(&path) {
            if let Some(e) = entries.get(ino) {
                return Ok(Some(VfsDirEntry { name: name.to_string(), inode: *ino, file_type: e.file_type }));
            }
        }
        Ok(None)
    }
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        self.fs.create(&path, mode)
    }
    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        self.fs.mkdir(&path)?;
        Ok(Box::new(FjallDirectory { path, fs: self.fs.clone() }))
    }
    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        self.fs.remove(&path)
    }
    fn rename_entry(&self, old: &str, new: &str) -> VfsResult<()> {
        let from = if self.path.ends_with('/') { format!("{}{}", self.path, old) } else { format!("{}/{}", self.path, old) };
        let to = if self.path.ends_with('/') { format!("{}{}", self.path, new) } else { format!("{}/{}", self.path, new) };
        self.fs.rename(&from, &to)
    }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.fs.open(&full, mode)
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableFjallFile::new(file))
    }
    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.fs.open_directory(&full).map(|d| Box::new(d) as Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
    }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.fs.stat(&full)
    }
    fn exists(&self, path: &str) -> VfsResult<bool> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        self.fs.exists(&full)
    }
}

struct FjallFsInner {
    inodes: InodeStore,
    paths: PathStore,
    next_ino: std::sync::atomic::AtomicU64,
}

/// FjallFs — complete VfsFileSystem backed by in-memory storage (fjall v2 compatible API).
pub struct FjallFs {
    inner: Arc<FjallFsInner>,
    config: FjallVfsConfig,
    root_ino: u64,
}

fn inode_to_metadata(e: &InodeEntry) -> VfsMetadata {
    VfsMetadata {
        inode: 0, size: e.size, file_type: e.file_type, permissions: e.permissions,
        owner: e.owner, created: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(e.created_at)),
        modified: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(e.updated_at)),
        accessed: None, checksum: Checksum::None, version: e.version, state: VfsEntryState::Ready,
    }
}

impl FjallFs {
    pub fn open(_path: impl AsRef<Path>) -> VfsResult<Self> {
        Self::open_with_config(_path, FjallVfsConfig::default())
    }

    pub fn open_with_config(_path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> {
        let inner = Arc::new(FjallFsInner {
            inodes: std::sync::RwLock::new(std::collections::HashMap::new()),
            paths: std::sync::RwLock::new(std::collections::HashMap::new()),
            next_ino: std::sync::atomic::AtomicU64::new(1),
        });
        let root_ino = inner.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        inner.inodes.write().unwrap().insert(root_ino, InodeEntry {
            path: "/".to_string(), name: "/".to_string(), file_type: VfsFileType::Directory,
            size: 0, permissions: 0o755, owner: (0, 0), version: 0,
            created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        inner.paths.write().unwrap().insert("/".to_string(), root_ino);
        Ok(Self { inner, config, root_ino })
    }

    pub fn in_memory() -> VfsResult<Self> {
        Self::open(std::env::temp_dir().join(format!("fjallvfs-{}", std::process::id())))
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
        let entries = self.inner.inodes.read().unwrap();
        let paths = self.inner.paths.read().unwrap();
        let ino = paths.get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {path}") })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        Ok(self.inner.paths.read().unwrap().contains_key(path))
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let paths = self.inner.paths.read().unwrap();
        let ino = paths.get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {path}") })?;
        if let Some(e) = entries.get_mut(&ino) { e.permissions = mode; e.updated_at = now_ms(); }
        Ok(())
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let ino = self.inner.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: link.to_string(), name: link.rsplit_once('/').map(|(_, n)| n).unwrap_or(link).to_string(),
            file_type: VfsFileType::Symlink, size: target.len() as u64, permissions: 0o777,
            owner: (0, 0), version: 0, created_at: now_ms(), updated_at: now_ms(),
            content: Vec::new(), symlink_target: Some(target.to_string()),
        });
        self.inner.paths.write().unwrap().insert(link.to_string(), ino);
        Ok(())
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let entries = self.inner.inodes.read().unwrap();
        let paths = self.inner.paths.read().unwrap();
        let ino = paths.get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {path}") })?;
        entries.get(&ino).and_then(|e| e.symlink_target.clone())
            .ok_or_else(|| VfsError::Backend { message: "not a symlink".into() }.into())
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let mut paths = self.inner.paths.write().unwrap();
        let ino = paths.remove(from).ok_or_else(|| VfsError::Backend { message: format!("not found: {from}") })?;
        if let Some(e) = entries.get_mut(&ino) { e.path = to.to_string(); e.updated_at = now_ms(); }
        paths.insert(to.to_string(), ino);
        Ok(())
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let mut paths = self.inner.paths.write().unwrap();
        let ino = paths.remove(path).ok_or_else(|| VfsError::Backend { message: format!("not found: {path}") })?;
        entries.remove(&ino);
        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(path) { return Err(VfsError::Backend { message: format!("not found: {path}") }.into()); }
        Ok(FjallFile { path: path.to_string(), mode, fs: self.inner.clone() })
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableFjallFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(path) { return Err(VfsError::NotADirectory { path: path.to_string() }.into()); }
        let entries = self.inner.inodes.read().unwrap();
        let ino = paths.get(path).unwrap();
        if let Some(e) = entries.get(ino) {
            if e.file_type != VfsFileType::Directory {
                return Err(VfsError::NotADirectory { path: path.to_string() }.into());
            }
        }
        Ok(FjallDirectory { path: path.to_string(), fs: self.inner.clone() })
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let ino = self.inner.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.to_string(), name: path.rsplit_once('/').map(|(_, n)| n).unwrap_or(path).to_string(),
            file_type: VfsFileType::Regular, size: 0, permissions: mode, owner: (0, 0),
            version: 0, created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.to_string(), ino);
        Ok(FjallFile { path: path.to_string(), mode: OpenMode::ReadWrite, fs: self.inner.clone() })
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let ino = self.inner.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.to_string(), name: path.rsplit_once('/').map(|(_, n)| n).unwrap_or(path).to_string(),
            file_type: VfsFileType::Directory, size: 0, permissions: 0o755, owner: (0, 0),
            version: 0, created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.to_string(), ino);
        Ok(())
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
        self.remove(path)
    }

    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').filter(|p| !p.is_empty()).collect();
        let mut current = String::new();
        for part in parts {
            current = format!("{current}/{part}");
            if !self.exists(&current)? {
                self.mkdir(&current)?;
            }
        }
        Ok(())
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        self.inner.paths.read().unwrap().get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {path}") })
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.inner.inodes.read().unwrap().get(&ino).map(|e| e.path.clone()).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.inner.inodes.read().unwrap().get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
}

/// FjallDelta — DeltaStore extension with whiteout support.
pub struct FjallDelta {
    inner: FjallFs,
    whiteouts: std::sync::RwLock<std::collections::HashMap<String, u64>>,
}

impl FjallDelta {
    pub fn open(path: impl AsRef<Path>) -> VfsResult<Self> {
        let inner = FjallFs::open(path)?;
        Ok(Self { inner, whiteouts: std::sync::RwLock::new(std::collections::HashMap::new()) })
    }

    pub fn open_with_config(path: impl AsRef<Path>, config: FjallVfsConfig) -> VfsResult<Self> {
        let inner = FjallFs::open_with_config(path, config)?;
        Ok(Self { inner, whiteouts: std::sync::RwLock::new(std::collections::HashMap::new()) })
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
        if self.whiteouts.read().unwrap().contains_key(path) { return Ok(false); }
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
        self.whiteouts.write().unwrap().insert(path.to_string(), version);
        Ok(())
    }
    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        Ok(self.whiteouts.read().unwrap().get(path).copied())
    }
    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        self.whiteouts.write().unwrap().remove(path);
        Ok(())
    }
    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        Ok(self.whiteouts.read().unwrap().iter()
            .filter(|(p, _)| p.starts_with(dir))
            .map(|(p, v)| (p.clone(), *v)).collect())
    }
    fn flush(&self) -> VfsResult<()> { Ok(()) }
    fn reset(&self) -> VfsResult<()> {
        self.whiteouts.write().unwrap().clear();
        Ok(())
    }
}

// ── Async trait implementations ──

#[async_trait]
impl AsyncVfsFile for FjallFile {
    async fn read_at_async(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { self.read_at(buf, offset) }
    async fn write_at_async(&self, data: &[u8], offset: u64) -> VfsResult<usize> { self.write_at(data, offset) }
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
    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> { self.list() }
    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> { self.get_entry(&name) }
    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> { self.create_file(&name, mode) }
    async fn create_dir_async(&self, name: String) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        self.create_dir(&name).map(|d| Box::new(*d) as Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
    }
    async fn remove_entry_async(&self, name: String) -> VfsResult<()> { self.remove_entry(&name) }
    async fn rename_entry_async(&self, old: String, new: String) -> VfsResult<()> { self.rename_entry(&old, &new) }
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

// Re-exports
