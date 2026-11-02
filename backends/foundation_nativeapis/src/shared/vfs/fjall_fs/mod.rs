#![cfg(feature = "vfs-fjall")]

//! `InodeFs` / `InodeDelta` — an inode-based in-memory VFS.
//!
//! These stores keep every node in memory; they are not backed by fjall. The
//! module keeps its historical `fjall_fs` name so existing imports do not move,
//! but the types are named after what they are: a POSIX-like in-memory file
//! system with inode tracking, symlinks, and seekable files.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::shared::vfs::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsFile, VfsDirectory, VfsFileSystem, SeekableVfsFile};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType, VfsMetadata, Checksum};

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[derive(Clone)]
struct InodeEntry {
    path: String,
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

fn inode_to_metadata(e: &InodeEntry) -> VfsMetadata {
    VfsMetadata {
        inode: 0, size: e.size, file_type: e.file_type, permissions: e.permissions,
        owner: e.owner,
        created: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(e.created_at)),
        modified: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(e.updated_at)),
        accessed: None, checksum: Checksum::None, version: e.version, state: VfsEntryState::Ready,
    }
}

struct InodeFsInner {
    inodes: RwLock<HashMap<u64, InodeEntry>>,
    paths: RwLock<HashMap<String, u64>>,
    next_ino: std::sync::atomic::AtomicU64,
}

impl InodeFsInner {
    fn new() -> Arc<Self> {
        let inner = Arc::new(Self {
            inodes: RwLock::new(HashMap::new()),
            paths: RwLock::new(HashMap::new()),
            next_ino: std::sync::atomic::AtomicU64::new(1),
        });
        let root_ino = inner.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        inner.inodes.write().unwrap().insert(root_ino, InodeEntry {
            path: "/".to_string(), file_type: VfsFileType::Directory,
            size: 0, permissions: 0o755, owner: (0, 0), version: 0,
            created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        inner.paths.write().unwrap().insert("/".to_string(), root_ino);
        inner
    }

    fn next_ino(&self) -> u64 {
        self.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

/// `InodeFs` — an in-memory `VfsFileSystem` with inode tracking.
#[derive(Clone)]
pub struct InodeFs {
    inner: Arc<InodeFsInner>,
}

impl InodeFs {
    /// Creates a new empty in-memory file system with a root directory.
    pub fn new() -> Self {
        Self { inner: InodeFsInner::new() }
    }
}

impl Default for InodeFs {
    fn default() -> Self {
        Self::new()
    }
}

pub struct InodeFile { path: String, mode: OpenMode, inner: Arc<InodeFsInner> }
pub struct SeekableInodeFile { file: InodeFile, pos: Arc<RwLock<u64>> }
pub struct InodeDir { path: String, inner: Arc<InodeFsInner> }

impl SeekableInodeFile { fn new(file: InodeFile) -> Self { Self { file, pos: Arc::new(RwLock::new(0)) } } }

impl VfsFile for InodeFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let entries = self.inner.inodes.read().unwrap();
        let ino = self.inner.paths.read().unwrap().get(&self.path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", self.path) })?;
        let entry = entries.get(&ino).ok_or_else(|| VfsError::Backend { message: "inode not found".into() })?;
        let off = offset as usize;
        if off >= entry.content.len() { return Ok(0); }
        let n = buf.len().min(entry.content.len() - off);
        buf[..n].copy_from_slice(&entry.content[off..off + n]);
        Ok(n)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read { return Err(VfsError::ReadOnly.into()); }
        let mut entries = self.inner.inodes.write().unwrap();
        let ino = self.inner.paths.read().unwrap().get(&self.path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", self.path) })?;
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
        let entries = self.inner.inodes.read().unwrap();
        let ino = self.inner.paths.read().unwrap().get(&self.path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", self.path) })?;
        entries.get(&ino).map(|e| e.size).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn truncate(&self, size: u64) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let ino = self.inner.paths.read().unwrap().get(&self.path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", self.path) })?;
        if let Some(entry) = entries.get_mut(&ino) {
            entry.content.truncate(size as usize);
            entry.size = entry.content.len() as u64;
            entry.updated_at = now_ms();
        }
        Ok(())
    }
    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let entries = self.inner.inodes.read().unwrap();
        let ino = self.inner.paths.read().unwrap().get(&self.path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", self.path) })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
}

impl VfsFile for SeekableInodeFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { self.file.read_at(buf, offset) }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> { self.file.write_at(buf, offset) }
    fn sync_data(&self) -> VfsResult<()> { self.file.sync_data() }
    fn size(&self) -> VfsResult<u64> { self.file.size() }
    fn truncate(&self, size: u64) -> VfsResult<()> { self.file.truncate(size) }
    fn metadata(&self) -> VfsResult<VfsMetadata> { self.file.metadata() }
}

impl SeekableVfsFile for SeekableInodeFile {
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

impl VfsDirectory for InodeDir {
    type File = InodeFile;
    type SeekableFile = SeekableInodeFile;
    fn path(&self) -> &str { &self.path }
    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let entries = self.inner.inodes.read().unwrap();
        let ino = self.inner.paths.read().unwrap().get(&self.path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", self.path) })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let entries = self.inner.inodes.read().unwrap();
        let prefix = if self.path.ends_with('/') { self.path.clone() } else { format!("{}/", self.path) };
        Ok(entries.iter().filter_map(|(ino, e)| {
            if e.path.starts_with(&prefix) {
                let remaining = e.path.strip_prefix(&prefix).unwrap_or(&e.path);
                if !remaining.contains('/') && !remaining.is_empty() {
                    Some(VfsDirEntry { name: remaining.to_string(), inode: *ino, file_type: e.file_type })
                } else { None }
            } else { None }
        }).collect())
    }
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        let paths = self.inner.paths.read().unwrap();
        let entries = self.inner.inodes.read().unwrap();
        if let Some(ino) = paths.get(&path) {
            if let Some(e) = entries.get(ino) {
                return Ok(Some(VfsDirEntry { name: name.to_string(), inode: *ino, file_type: e.file_type }));
            }
        }
        Ok(None)
    }
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        let ino = self.inner.next_ino();
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.clone(), file_type: VfsFileType::Regular,
            size: 0, permissions: mode, owner: (0, 0), version: 0,
            created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.clone(), ino);
        Ok(InodeFile { path, mode: OpenMode::ReadWrite, inner: self.inner.clone() })
    }
    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        let ino = self.inner.next_ino();
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.clone(), file_type: VfsFileType::Directory,
            size: 0, permissions: 0o755, owner: (0, 0), version: 0,
            created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.clone(), ino);
        Ok(Box::new(InodeDir { path, inner: self.inner.clone() }))
    }
    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        let mut entries = self.inner.inodes.write().unwrap();
        let mut paths = self.inner.paths.write().unwrap();
        if let Some(ino) = paths.remove(&path) { entries.remove(&ino); }
        Ok(())
    }
    fn rename_entry(&self, old: &str, new: &str) -> VfsResult<()> {
        let from = if self.path.ends_with('/') { format!("{}{}", self.path, old) } else { format!("{}/{}", self.path, old) };
        let to = if self.path.ends_with('/') { format!("{}{}", self.path, new) } else { format!("{}/{}", self.path, new) };
        let mut entries = self.inner.inodes.write().unwrap();
        let mut paths = self.inner.paths.write().unwrap();
        if let Some(ino) = paths.remove(&from) {
            if let Some(e) = entries.get_mut(&ino) { e.path = to.clone(); e.updated_at = now_ms(); }
            paths.insert(to, ino);
        }
        Ok(())
    }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(&full) { return Err(VfsError::Backend { message: format!("not found: {}", full) }.into()); }
        Ok(InodeFile { path: full, mode, inner: self.inner.clone() })
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableInodeFile::new(file))
    }
    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(&full) { return Err(VfsError::NotADirectory { path: full }.into()); }
        Ok(Box::new(InodeDir { path: full, inner: self.inner.clone() }))
    }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        let entries = self.inner.inodes.read().unwrap();
        let paths = self.inner.paths.read().unwrap();
        let ino = paths.get(&full).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {}", full) })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn exists(&self, path: &str) -> VfsResult<bool> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        Ok(self.inner.paths.read().unwrap().contains_key(&full))
    }
}

impl VfsFileSystem for InodeFs {
    type File = InodeFile;
    type SeekableFile = SeekableInodeFile;
    type Directory = InodeDir;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities { seekable: true, symlinks: true, permissions_enforced: false, event_emission: false, persistent: true }
    }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let entries = self.inner.inodes.read().unwrap();
        let paths = self.inner.paths.read().unwrap();
        let ino = paths.get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {}", path) })?;
        entries.get(&ino).map(inode_to_metadata).ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn exists(&self, path: &str) -> VfsResult<bool> { Ok(self.inner.paths.read().unwrap().contains_key(path)) }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let paths = self.inner.paths.read().unwrap();
        let ino = paths.get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {}", path) })?;
        if let Some(e) = entries.get_mut(&ino) { e.permissions = mode; e.updated_at = now_ms(); }
        Ok(())
    }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let ino = self.inner.next_ino();
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: link.to_string(),
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
        let ino = paths.get(path).copied().ok_or_else(|| VfsError::Backend { message: format!("not found: {}", path) })?;
        entries.get(&ino).and_then(|e| e.symlink_target.clone())
            .ok_or_else(|| VfsError::Backend { message: "not a symlink".into() }.into())
    }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let mut paths = self.inner.paths.write().unwrap();
        let ino = paths.remove(from).ok_or_else(|| VfsError::Backend { message: format!("not found: {}", from) })?;
        if let Some(e) = entries.get_mut(&ino) { e.path = to.to_string(); e.updated_at = now_ms(); }
        paths.insert(to.to_string(), ino);
        Ok(())
    }
    fn remove(&self, path: &str) -> VfsResult<()> {
        let mut entries = self.inner.inodes.write().unwrap();
        let mut paths = self.inner.paths.write().unwrap();
        let ino = paths.remove(path).ok_or_else(|| VfsError::Backend { message: format!("not found: {}", path) })?;
        entries.remove(&ino);
        Ok(())
    }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(path) { return Err(VfsError::Backend { message: format!("not found: {}", path) }.into()); }
        Ok(InodeFile { path: path.to_string(), mode, inner: self.inner.clone() })
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableInodeFile::new(file))
    }
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(path) { return Err(VfsError::NotADirectory { path: path.to_string() }.into()); }
        let entries = self.inner.inodes.read().unwrap();
        let ino = paths.get(path).unwrap();
        if let Some(e) = entries.get(ino) {
            if e.file_type != VfsFileType::Directory { return Err(VfsError::NotADirectory { path: path.to_string() }.into()); }
        }
        Ok(InodeDir { path: path.to_string(), inner: self.inner.clone() })
    }
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        // Check if parent directory exists
        if let Some(parent) = path.rsplit_once('/') {
            let parent_path = if parent.0.is_empty() { "/" } else { parent.0 };
            if !self.inner.paths.read().unwrap().contains_key(parent_path) {
                return Err(VfsError::Backend { message: format!("parent directory not found: {}", parent_path) }.into());
            }
        }
        let ino = self.inner.next_ino();
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.to_string(), file_type: VfsFileType::Regular, size: 0, permissions: mode, owner: (0, 0),
            version: 0, created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.to_string(), ino);
        Ok(InodeFile { path: path.to_string(), mode: OpenMode::ReadWrite, inner: self.inner.clone() })
    }
    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let ino = self.inner.next_ino();
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.to_string(), file_type: VfsFileType::Directory, size: 0, permissions: 0o755, owner: (0, 0),
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
    fn remove_all(&self, path: &str) -> VfsResult<()> { self.remove(path) }
    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').filter(|p| !p.is_empty()).collect();
        let mut current = String::new();
        for part in parts { current = format!("{current}/{part}"); if !self.exists(&current)? { self.mkdir(&current)?; } }
        Ok(())
    }
    fn inode(&self, path: &str) -> VfsResult<u64> {
        self.inner.paths.read().unwrap().get(path).copied()
            .ok_or_else(|| VfsError::Backend { message: format!("not found: {}", path) }.into())
    }
    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.inner.inodes.read().unwrap().get(&ino).map(|e| e.path.clone())
            .ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.inner.inodes.read().unwrap().get(&ino).map(inode_to_metadata)
            .ok_or_else(|| VfsError::Backend { message: "not found".into() }.into())
    }
}

pub struct InodeDelta { inner: InodeFs, whiteouts: Arc<RwLock<HashMap<String, u64>>> }

impl InodeDelta {
    /// Creates a new empty in-memory delta store.
    pub fn new() -> Self {
        Self { inner: InodeFs::new(), whiteouts: Arc::new(RwLock::new(HashMap::new())) }
    }
}

impl Default for InodeDelta {
    fn default() -> Self {
        Self::new()
    }
}

impl VfsFileSystem for InodeDelta {
    type File = InodeFile; type SeekableFile = SeekableInodeFile; type Directory = InodeDir;
    fn capabilities(&self) -> VfsCapabilities { VfsFileSystem::capabilities(&self.inner) }
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

impl DeltaStore for InodeDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        self.whiteouts.write().unwrap().insert(path.to_string(), version); Ok(())
    }
    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        Ok(self.whiteouts.read().unwrap().get(path).copied())
    }
    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        self.whiteouts.write().unwrap().remove(path); Ok(())
    }
    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        Ok(self.whiteouts.read().unwrap().iter().filter(|(p, _)| p.starts_with(dir)).map(|(p, v)| (p.clone(), *v)).collect())
    }
    fn flush(&self) -> VfsResult<()> { Ok(()) }
    fn reset(&self) -> VfsResult<()> { self.whiteouts.write().unwrap().clear(); Ok(()) }
}

// ──────────────────────────────────────────────
// Async trait implementations (sync-native: delegate to sync)
// ──────────────────────────────────────────────

#[async_trait]
impl AsyncVfsFile for InodeFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        let mut buf = vec![0; len];
        let n = VfsFile::read_at(self, &mut buf, offset)?;
        buf.truncate(n);
        Ok(buf)
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        VfsFile::write_at(self, &data, offset)
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        VfsFile::sync_data(self)
    }

    async fn size_async(&self) -> VfsResult<u64> {
        VfsFile::size(self)
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        VfsFile::truncate(self, size)
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        VfsFile::metadata(self)
    }
}

#[async_trait]
impl AsyncVfsFile for SeekableInodeFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        let mut buf = vec![0; len];
        let n = VfsFile::read_at(self, &mut buf, offset)?;
        buf.truncate(n);
        Ok(buf)
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        VfsFile::write_at(self, &data, offset)
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        VfsFile::sync_data(self)
    }

    async fn size_async(&self) -> VfsResult<u64> {
        VfsFile::size(self)
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        VfsFile::truncate(self, size)
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        VfsFile::metadata(self)
    }
}

#[async_trait]
impl AsyncSeekableVfsFile for SeekableInodeFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let mut buf = vec![0; len];
        let n = SeekableVfsFile::read(self, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        SeekableVfsFile::write(self, &data)
    }

    async fn seek_async(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> {
        SeekableVfsFile::seek(self, pos)
    }

    fn position_async(&self) -> u64 {
        SeekableVfsFile::position(self)
    }
}

#[async_trait]
impl AsyncVfsDirectory for InodeDir {
    type File = InodeFile;
    type SeekableFile = SeekableInodeFile;

    fn path(&self) -> String {
        VfsDirectory::path(self).to_string()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        VfsDirectory::metadata(self)
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        VfsDirectory::list(self)
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        VfsDirectory::get_entry(self, &name)
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> {
        VfsDirectory::create_file(self, &name, mode)
    }

    async fn create_dir_async(
        &self,
        name: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let dir = VfsDirectory::create_dir(self, &name)?;
        Ok(Box::new(AsyncInodeDirectory::from_sync(dir)))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        VfsDirectory::remove_entry(self, &name)
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        VfsDirectory::rename_entry(self, &old_name, &new_name)
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        VfsDirectory::open(self, &path, mode)
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        VfsDirectory::open_seekable(self, &path, mode)
    }

    async fn open_directory_async(
        &self,
        path: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let dir = VfsDirectory::open_directory(self, &path)?;
        Ok(Box::new(AsyncInodeDirectory::from_sync(dir)))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        VfsDirectory::stat(self, &path)
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        VfsDirectory::exists(self, &path)
    }
}

/// Thin wrapper that makes a `Box<dyn VfsDirectory>` implement
/// `AsyncVfsDirectory`. Used when the sync-native `InodeDir` creates child
/// directories — the returned `Box<dyn VfsDirectory>` must be re-wrapped as an
/// async directory.
pub struct AsyncInodeDirectory {
    inner: Box<dyn VfsDirectory<File = InodeFile, SeekableFile = SeekableInodeFile>>,
    cached_path: String,
}

impl AsyncInodeDirectory {
    fn from_sync(dir: Box<dyn VfsDirectory<File = InodeFile, SeekableFile = SeekableInodeFile>>) -> Self {
        let cached_path = dir.path().to_string();
        Self { inner: dir, cached_path }
    }
}

#[async_trait]
impl AsyncVfsDirectory for AsyncInodeDirectory {
    type File = InodeFile;
    type SeekableFile = SeekableInodeFile;

    fn path(&self) -> String {
        self.cached_path.clone()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata()
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        self.inner.list()
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        self.inner.get_entry(&name)
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> {
        self.inner.create_file(&name, mode)
    }

    async fn create_dir_async(
        &self,
        name: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let dir = self.inner.create_dir(&name)?;
        Ok(Box::new(Self::from_sync(dir)))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        self.inner.remove_entry(&name)
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        self.inner.rename_entry(&old_name, &new_name)
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        self.inner.open(&path, mode)
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        self.inner.open_seekable(&path, mode)
    }

    async fn open_directory_async(
        &self,
        path: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let dir = self.inner.open_directory(&path)?;
        Ok(Box::new(Self::from_sync(dir)))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.inner.stat(&path)
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.inner.exists(&path)
    }
}

#[async_trait]
impl AsyncVfsFileSystem for InodeFs {
    type File = InodeFile;
    type SeekableFile = SeekableInodeFile;
    type Directory = AsyncInodeDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsFileSystem::capabilities(self)
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        VfsFileSystem::stat(self, &path)
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        VfsFileSystem::exists(self, &path)
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        VfsFileSystem::chmod(self, &path, mode)
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        VfsFileSystem::symlink(self, &target, &link)
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        VfsFileSystem::readlink(self, &path)
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        VfsFileSystem::rename(self, &from, &to)
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        VfsFileSystem::remove(self, &path)
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        VfsFileSystem::open(self, &path, mode)
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        VfsFileSystem::open_seekable(self, &path, mode)
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        let dir = VfsFileSystem::open_directory(self, &path)?;
        Ok(AsyncInodeDirectory::from_sync(Box::new(dir)))
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        VfsFileSystem::create(self, &path, mode)
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        VfsFileSystem::mkdir(self, &path)
    }
}

#[async_trait]
impl AsyncVfsFileSystem for InodeDelta {
    type File = InodeFile;
    type SeekableFile = SeekableInodeFile;
    type Directory = AsyncInodeDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsFileSystem::capabilities(self)
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        VfsFileSystem::stat(self, &path)
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        VfsFileSystem::exists(self, &path)
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        VfsFileSystem::chmod(self, &path, mode)
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        VfsFileSystem::symlink(self, &target, &link)
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        VfsFileSystem::readlink(self, &path)
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        VfsFileSystem::rename(self, &from, &to)
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        VfsFileSystem::remove(self, &path)
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        VfsFileSystem::open(self, &path, mode)
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        VfsFileSystem::open_seekable(self, &path, mode)
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        <InodeFs as AsyncVfsFileSystem>::open_directory_async(&self.inner, path).await
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        VfsFileSystem::create(self, &path, mode)
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        VfsFileSystem::mkdir(self, &path)
    }
}

#[async_trait]
impl AsyncDeltaStore for InodeDelta {
    async fn add_whiteout_async(&self, path: String, version: u64) -> VfsResult<()> {
        DeltaStore::add_whiteout(self, &path, version)
    }

    async fn is_whiteout_async(&self, path: String) -> VfsResult<Option<u64>> {
        DeltaStore::is_whiteout(self, &path)
    }

    async fn remove_whiteout_async(&self, path: String) -> VfsResult<()> {
        DeltaStore::remove_whiteout(self, &path)
    }

    async fn list_whiteouts_async(&self, dir: String) -> VfsResult<Vec<(String, u64)>> {
        DeltaStore::list_whiteouts(self, &dir)
    }

    async fn flush_async(&self) -> VfsResult<()> {
        DeltaStore::flush(self)
    }

    async fn reset_async(&self) -> VfsResult<()> {
        DeltaStore::reset(self)
    }
}
