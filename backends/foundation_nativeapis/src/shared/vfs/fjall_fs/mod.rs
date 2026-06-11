#![cfg(feature = "vfs-fjall")]

//! FjallFs / FjallDelta — simplified in-memory VFS (fjall-compatible API).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsFile, VfsDirectory, VfsFileSystem, SeekableVfsFile};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType, VfsMetadata, Checksum};

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

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

fn inode_to_metadata(e: &InodeEntry) -> VfsMetadata {
    VfsMetadata {
        inode: 0, size: e.size, file_type: e.file_type, permissions: e.permissions,
        owner: e.owner,
        created: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(e.created_at)),
        modified: Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(e.updated_at)),
        accessed: None, checksum: Checksum::None, version: e.version, state: VfsEntryState::Ready,
    }
}

struct FjallFsInner {
    inodes: RwLock<HashMap<u64, InodeEntry>>,
    paths: RwLock<HashMap<String, u64>>,
    next_ino: std::sync::atomic::AtomicU64,
}

impl FjallFsInner {
    fn new() -> Arc<Self> {
        let inner = Arc::new(Self {
            inodes: RwLock::new(HashMap::new()),
            paths: RwLock::new(HashMap::new()),
            next_ino: std::sync::atomic::AtomicU64::new(1),
        });
        let root_ino = inner.next_ino.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        inner.inodes.write().unwrap().insert(root_ino, InodeEntry {
            path: "/".to_string(), name: "/".to_string(), file_type: VfsFileType::Directory,
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

/// FjallFs — VfsFileSystem with in-memory storage.
pub struct FjallFs {
    inner: Arc<FjallFsInner>,
}

/// FjallVfsConfig — placeholder config.
#[derive(Clone, Debug, Default)]
pub struct FjallVfsConfig;

impl FjallFs {
    pub fn open(_path: impl AsRef<std::path::Path>) -> VfsResult<Self> {
        Ok(Self { inner: FjallFsInner::new() })
    }
    pub fn open_with_config(_path: impl AsRef<std::path::Path>, _config: FjallVfsConfig) -> VfsResult<Self> {
        Self::open("/tmp")
    }
    pub fn in_memory() -> VfsResult<Self> { Self::open("/tmp") }
}

pub struct FjallFile { path: String, mode: OpenMode, inner: Arc<FjallFsInner> }
pub struct SeekableFjallFile { file: FjallFile, pos: Arc<RwLock<u64>> }
pub struct FjallDirectory { path: String, inner: Arc<FjallFsInner> }

impl SeekableFjallFile { pub fn new(file: FjallFile) -> Self { Self { file, pos: Arc::new(RwLock::new(0)) } } }
impl FjallDirectory { pub fn new(path: String, inner: Arc<FjallFsInner>) -> Self { Self { path, inner } } }

impl VfsFile for FjallFile {
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

impl VfsDirectory for FjallDirectory {
    type File = FjallFile;
    type SeekableFile = SeekableFjallFile;
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
            path: path.clone(), name: name.to_string(), file_type: VfsFileType::Regular,
            size: 0, permissions: mode, owner: (0, 0), version: 0,
            created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.clone(), ino);
        Ok(FjallFile { path, mode: OpenMode::ReadWrite, inner: self.inner.clone() })
    }
    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let path = if self.path.ends_with('/') { format!("{}{}", self.path, name) } else { format!("{}/{}", self.path, name) };
        let ino = self.inner.next_ino();
        self.inner.inodes.write().unwrap().insert(ino, InodeEntry {
            path: path.clone(), name: name.to_string(), file_type: VfsFileType::Directory,
            size: 0, permissions: 0o755, owner: (0, 0), version: 0,
            created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.clone(), ino);
        Ok(Box::new(FjallDirectory { path, inner: self.inner.clone() }))
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
        Ok(FjallFile { path: full, mode, inner: self.inner.clone() })
    }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableFjallFile::new(file))
    }
    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let full = if path.starts_with('/') { path.to_string() } else if self.path.ends_with('/') { format!("{}{}", self.path, path) } else { format!("{}/{}", self.path, path) };
        let paths = self.inner.paths.read().unwrap();
        if !paths.contains_key(&full) { return Err(VfsError::NotADirectory { path: full }.into()); }
        Ok(Box::new(FjallDirectory { path: full, inner: self.inner.clone() }))
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
        Ok(FjallFile { path: path.to_string(), mode, inner: self.inner.clone() })
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
            if e.file_type != VfsFileType::Directory { return Err(VfsError::NotADirectory { path: path.to_string() }.into()); }
        }
        Ok(FjallDirectory { path: path.to_string(), inner: self.inner.clone() })
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
            path: path.to_string(), name: path.rsplit_once('/').map(|(_, n)| n).unwrap_or(path).to_string(),
            file_type: VfsFileType::Regular, size: 0, permissions: mode, owner: (0, 0),
            version: 0, created_at: now_ms(), updated_at: now_ms(), content: Vec::new(), symlink_target: None,
        });
        self.inner.paths.write().unwrap().insert(path.to_string(), ino);
        Ok(FjallFile { path: path.to_string(), mode: OpenMode::ReadWrite, inner: self.inner.clone() })
    }
    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let ino = self.inner.next_ino();
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

pub struct FjallDelta { inner: FjallFs, whiteouts: Arc<RwLock<HashMap<String, u64>>> }

impl FjallDelta {
    pub fn open(path: impl AsRef<std::path::Path>) -> VfsResult<Self> {
        Ok(Self { inner: FjallFs::open(path)?, whiteouts: Arc::new(RwLock::new(HashMap::new())) })
    }
    pub fn open_with_config(path: impl AsRef<std::path::Path>, config: FjallVfsConfig) -> VfsResult<Self> {
        Ok(Self { inner: FjallFs::open_with_config(path, config)?, whiteouts: Arc::new(RwLock::new(HashMap::new())) })
    }
    pub fn in_memory() -> VfsResult<Self> { Self::open("/tmp") }
}

impl VfsFileSystem for FjallDelta {
    type File = FjallFile; type SeekableFile = SeekableFjallFile; type Directory = FjallDirectory;
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
