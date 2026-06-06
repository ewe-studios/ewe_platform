use std::collections::HashMap;
use std::io::SeekFrom;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use foundation_errstacks::ErrorTrace;

use super::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use super::error::{VfsError, VfsResult};
use super::path_utils::{file_name, normalize_vfs_path, parent_path};
use super::traits::{SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use super::types::{
    OpenMode, VfsCapabilities, VfsDirEntry, VfsFileType, VfsMetadata,
};

const MAX_SYMLINK_HOPS: usize = 40;
const DEFAULT_DIR_PERMS: u32 = 0o755;

fn normalize_path(path: &str) -> VfsResult<String> {
    normalize_vfs_path(path)
}

#[derive(Debug, Clone)]
enum MemoryNode {
    File {
        content: Arc<RwLock<Vec<u8>>>,
        metadata: VfsMetadata,
    },
    Directory {
        metadata: VfsMetadata,
    },
    Symlink {
        target: String,
        metadata: VfsMetadata,
    },
}

impl MemoryNode {
    fn metadata(&self) -> &VfsMetadata {
        match self {
            MemoryNode::File { metadata, .. } => metadata,
            MemoryNode::Directory { metadata } => metadata,
            MemoryNode::Symlink { metadata, .. } => metadata,
        }
    }

    fn metadata_mut(&mut self) -> &mut VfsMetadata {
        match self {
            MemoryNode::File { metadata, .. } => metadata,
            MemoryNode::Directory { metadata } => metadata,
            MemoryNode::Symlink { metadata, .. } => metadata,
        }
    }

    fn file_type(&self) -> VfsFileType {
        match self {
            MemoryNode::File { .. } => VfsFileType::Regular,
            MemoryNode::Directory { .. } => VfsFileType::Directory,
            MemoryNode::Symlink { .. } => VfsFileType::Symlink,
        }
    }
}

#[derive(Debug)]
struct MemoryFsInner {
    nodes: HashMap<String, MemoryNode>,
    version: u64,
}

impl MemoryFsInner {
    fn next_version(&mut self) -> u64 {
        self.version += 1;
        self.version
    }

    fn resolve_symlinks(&self, path: &str) -> VfsResult<String> {
        let mut current = path.to_string();
        for _ in 0..MAX_SYMLINK_HOPS {
            match self.nodes.get(&current) {
                Some(MemoryNode::Symlink { target, .. }) => {
                    current = if target.starts_with('/') {
                        normalize_path(target)?
                    } else {
                        let parent = parent_path(&current).unwrap_or_else(|| "/".to_string());
                        normalize_path(&format!("{parent}/{target}"))?
                    };
                }
                _ => return Ok(current),
            }
        }
        Err(ErrorTrace::new(VfsError::SymlinkLoop {
            path: path.to_string(),
        }))
    }

    fn list_children(&self, dir_path: &str) -> Vec<VfsDirEntry> {
        let prefix = if dir_path == "/" {
            "/".to_string()
        } else {
            format!("{dir_path}/")
        };
        let mut entries = Vec::new();
        for (key, node) in &self.nodes {
            if key == dir_path {
                continue;
            }
            if let Some(rest) = key.strip_prefix(&prefix) {
                if !rest.contains('/') && !rest.is_empty() {
                    entries.push(VfsDirEntry {
                        name: rest.to_string(),
                        file_type: node.file_type(),
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }
}

#[derive(Debug, Clone)]
pub struct MemoryFs {
    inner: Arc<RwLock<MemoryFsInner>>,
}

impl MemoryFs {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            "/".to_string(),
            MemoryNode::Directory {
                metadata: VfsMetadata::new_directory(DEFAULT_DIR_PERMS),
            },
        );
        Self {
            inner: Arc::new(RwLock::new(MemoryFsInner { nodes, version: 0 })),
        }
    }
}

impl Default for MemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MemoryFile {
    content: Arc<RwLock<Vec<u8>>>,
    path: String,
    fs: Arc<RwLock<MemoryFsInner>>,
    mode: OpenMode,
}

impl VfsFile for MemoryFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let content = self.content.read().unwrap();
        let offset = offset as usize;
        if offset >= content.len() {
            return Ok(0);
        }
        let available = content.len() - offset;
        let to_copy = buf.len().min(available);
        buf[..to_copy].copy_from_slice(&content[offset..offset + to_copy]);
        Ok(to_copy)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let mut content = self.content.write().unwrap();
        let offset = offset as usize;
        let needed = offset + buf.len();
        if needed > content.len() {
            content.resize(needed, 0);
        }
        content[offset..offset + buf.len()].copy_from_slice(buf);

        let mut inner = self.fs.write().unwrap();
        let version = inner.next_version();
        if let Some(node) = inner.nodes.get_mut(&self.path) {
            let meta = node.metadata_mut();
            meta.size = content.len() as u64;
            meta.version = version;
            meta.modified = Some(std::time::SystemTime::now());
        }
        Ok(buf.len())
    }

    fn sync_data(&self) -> VfsResult<()> {
        Ok(())
    }

    fn size(&self) -> VfsResult<u64> {
        let content = self.content.read().unwrap();
        Ok(content.len() as u64)
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let mut content = self.content.write().unwrap();
        content.resize(size as usize, 0);

        let mut inner = self.fs.write().unwrap();
        let version = inner.next_version();
        if let Some(node) = inner.nodes.get_mut(&self.path) {
            let meta = node.metadata_mut();
            meta.size = size;
            meta.version = version;
            meta.modified = Some(std::time::SystemTime::now());
        }
        Ok(())
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let inner = self.fs.read().unwrap();
        match inner.nodes.get(&self.path) {
            Some(node) => Ok(node.metadata().clone()),
            None => Err(ErrorTrace::new(VfsError::NotFound {
                path: self.path.clone(),
            })),
        }
    }
}

// MemoryFile is Send+Sync because its fields are Arc<RwLock<_>> and OpenMode (Copy).
unsafe impl Send for MemoryFile {}
unsafe impl Sync for MemoryFile {}

pub struct SeekableMemoryFile {
    inner: MemoryFile,
    position: u64,
}

impl VfsFile for SeekableMemoryFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        VfsFile::read_at(&self.inner, buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        VfsFile::write_at(&self.inner, buf, offset)
    }

    fn sync_data(&self) -> VfsResult<()> {
        VfsFile::sync_data(&self.inner)
    }

    fn size(&self) -> VfsResult<u64> {
        VfsFile::size(&self.inner)
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        VfsFile::truncate(&self.inner, size)
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        VfsFile::metadata(&self.inner)
    }
}

impl SeekableVfsFile for SeekableMemoryFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = VfsFile::read_at(&self.inner, buf, self.position)?;
        self.position += n as u64;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let n = VfsFile::write_at(&self.inner, buf, self.position)?;
        self.position += n as u64;
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let size = VfsFile::size(&self.inner)? as i64;
        let new_pos = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => size + n,
            SeekFrom::Current(n) => self.position as i64 + n,
        };
        if new_pos < 0 {
            return Err(ErrorTrace::new(VfsError::InvalidPath {
                path: format!("seek to negative position: {new_pos}"),
            }));
        }
        self.position = new_pos as u64;
        Ok(self.position)
    }

    fn position(&self) -> u64 {
        self.position
    }
}

unsafe impl Send for SeekableMemoryFile {}
unsafe impl Sync for SeekableMemoryFile {}

pub struct MemoryDirectory {
    fs: Arc<RwLock<MemoryFsInner>>,
    dir_path: String,
}

impl MemoryDirectory {
    fn resolve_child_path(&self, name: &str) -> VfsResult<String> {
        if name == "." || name.is_empty() {
            return Ok(self.dir_path.clone());
        }
        if name.starts_with('/') {
            return normalize_path(name);
        }
        if name.contains("..") {
            return Err(ErrorTrace::new(VfsError::InvalidPath {
                path: format!("path traversal not allowed: {name}"),
            }));
        }
        Ok(if self.dir_path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.dir_path)
        })
    }
}

impl VfsDirectory for MemoryDirectory {
    type File = MemoryFile;
    type SeekableFile = SeekableMemoryFile;

    fn path(&self) -> &str {
        &self.dir_path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let inner = self.fs.read().unwrap();
        match inner.nodes.get(&self.dir_path) {
            Some(node) => Ok(node.metadata().clone()),
            None => Err(ErrorTrace::new(VfsError::NotFound {
                path: self.dir_path.clone(),
            })),
        }
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let inner = self.fs.read().unwrap();
        if !inner.nodes.contains_key(&self.dir_path) {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: self.dir_path.clone(),
            }));
        }
        Ok(inner.list_children(&self.dir_path))
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let child_path = self.resolve_child_path(name)?;
        let inner = self.fs.read().unwrap();
        let resolved = inner.resolve_symlinks(&child_path)?;
        match inner.nodes.get(&resolved) {
            Some(node) => Ok(Some(VfsDirEntry {
                name: file_name(&child_path).to_string(),
                file_type: node.file_type(),
            })),
            None => Ok(None),
        }
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let child_path = self.resolve_child_path(name)?;
        let mut inner = self.fs.write().unwrap();
        if inner.nodes.contains_key(&child_path) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: child_path,
            }));
        }
        let version = inner.next_version();
        let content = Arc::new(RwLock::new(Vec::new()));
        let mut metadata = VfsMetadata::new_file(0, mode);
        metadata.version = version;
        inner.nodes.insert(
            child_path.clone(),
            MemoryNode::File {
                content: content.clone(),
                metadata,
            },
        );
        Ok(MemoryFile {
            content,
            path: child_path,
            fs: self.fs.clone(),
            mode: OpenMode::ReadWrite,
        })
    }

    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let child_path = self.resolve_child_path(name)?;
        let mut inner = self.fs.write().unwrap();
        if inner.nodes.contains_key(&child_path) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: child_path,
            }));
        }
        let version = inner.next_version();
        let mut metadata = VfsMetadata::new_directory(DEFAULT_DIR_PERMS);
        metadata.version = version;
        inner
            .nodes
            .insert(child_path.clone(), MemoryNode::Directory { metadata });
        Ok(Box::new(MemoryDirectory {
            fs: self.fs.clone(),
            dir_path: child_path,
        }))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let child_path = self.resolve_child_path(name)?;
        let mut inner = self.fs.write().unwrap();
        let resolved = inner.resolve_symlinks(&child_path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::Directory { .. }) => {
                let children = inner.list_children(&resolved);
                if !children.is_empty() {
                    return Err(ErrorTrace::new(VfsError::NotAFile {
                        path: format!("directory not empty: {resolved}"),
                    }));
                }
            }
            Some(_) => {}
            None => {
                return Err(ErrorTrace::new(VfsError::NotFound { path: resolved }));
            }
        }
        // Remove the original symlink entry if path != resolved
        if child_path != resolved {
            inner.nodes.remove(&child_path);
        }
        inner.nodes.remove(&resolved);
        inner.next_version();
        Ok(())
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let old_path = self.resolve_child_path(old_name)?;
        let new_path = self.resolve_child_path(new_name)?;
        let mut inner = self.fs.write().unwrap();
        if !inner.nodes.contains_key(&old_path) {
            return Err(ErrorTrace::new(VfsError::NotFound { path: old_path }));
        }
        if inner.nodes.contains_key(&new_path) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: new_path }));
        }
        // Collect all paths to move (old_path itself + all descendants for dirs)
        let old_prefix = format!("{old_path}/");
        let keys_to_move: Vec<String> = inner
            .nodes
            .keys()
            .filter(|k| *k == &old_path || k.starts_with(&old_prefix))
            .cloned()
            .collect();

        let version = inner.next_version();
        for key in keys_to_move {
            let node = inner.nodes.remove(&key).unwrap();
            let new_key = if key == old_path {
                new_path.clone()
            } else {
                format!("{new_path}{}", &key[old_path.len()..])
            };
            inner.nodes.insert(new_key, node);
        }

        // Update version on the moved entry
        if let Some(node) = inner.nodes.get_mut(&new_path) {
            node.metadata_mut().version = version;
        }
        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let full_path = self.resolve_child_path(path)?;
        let inner = self.fs.read().unwrap();
        let resolved = inner.resolve_symlinks(&full_path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::File { content, .. }) => Ok(MemoryFile {
                content: content.clone(),
                path: resolved,
                fs: self.fs.clone(),
                mode,
            }),
            Some(_) => Err(ErrorTrace::new(VfsError::NotAFile { path: resolved })),
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = VfsDirectory::open(self, path, mode)?;
        Ok(SeekableMemoryFile {
            inner: file,
            position: 0,
        })
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let full_path = self.resolve_child_path(path)?;
        let inner = self.fs.read().unwrap();
        let resolved = inner.resolve_symlinks(&full_path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::Directory { .. }) => Ok(Box::new(MemoryDirectory {
                fs: self.fs.clone(),
                dir_path: resolved,
            })),
            Some(_) => Err(ErrorTrace::new(VfsError::NotADirectory { path: resolved })),
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let full_path = self.resolve_child_path(path)?;
        let inner = self.fs.read().unwrap();
        let resolved = inner.resolve_symlinks(&full_path)?;
        match inner.nodes.get(&resolved) {
            Some(node) => Ok(node.metadata().clone()),
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let full_path = self.resolve_child_path(path)?;
        let inner = self.fs.read().unwrap();
        match inner.resolve_symlinks(&full_path) {
            Ok(resolved) => Ok(inner.nodes.contains_key(&resolved)),
            Err(_) => Ok(false),
        }
    }
}

unsafe impl Send for MemoryDirectory {}
unsafe impl Sync for MemoryDirectory {}

impl VfsFileSystem for MemoryFs {
    type File = MemoryFile;
    type SeekableFile = SeekableMemoryFile;
    type Directory = MemoryDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities {
            seekable: true,
            symlinks: true,
            permissions_enforced: false,
            event_emission: false,
            persistent: false,
        }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let path = normalize_path(path)?;
        let inner = self.inner.read().unwrap();
        let resolved = inner.resolve_symlinks(&path)?;
        match inner.nodes.get(&resolved) {
            Some(node) => {
                let mut meta = node.metadata().clone();
                if let MemoryNode::File { content, .. } = node {
                    meta.size = content.read().unwrap().len() as u64;
                }
                Ok(meta)
            }
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let path = normalize_path(path)?;
        let inner = self.inner.read().unwrap();
        match inner.resolve_symlinks(&path) {
            Ok(resolved) => Ok(inner.nodes.contains_key(&resolved)),
            Err(_) => Ok(false),
        }
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let path = normalize_path(path)?;
        let mut inner = self.inner.write().unwrap();
        let resolved = inner.resolve_symlinks(&path)?;
        let version = inner.next_version();
        match inner.nodes.get_mut(&resolved) {
            Some(node) => {
                let meta = node.metadata_mut();
                meta.permissions = mode;
                meta.version = version;
                Ok(())
            }
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let link = normalize_path(link)?;
        let mut inner = self.inner.write().unwrap();
        if inner.nodes.contains_key(&link) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: link }));
        }
        let parent = parent_path(&link);
        if let Some(ref p) = parent {
            if !inner.nodes.contains_key(p) {
                return Err(ErrorTrace::new(VfsError::NotFound { path: p.clone() }));
            }
        }
        let version = inner.next_version();
        let mut metadata = VfsMetadata::new_symlink();
        metadata.version = version;
        inner.nodes.insert(
            link,
            MemoryNode::Symlink {
                target: target.to_string(),
                metadata,
            },
        );
        Ok(())
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let path = normalize_path(path)?;
        let inner = self.inner.read().unwrap();
        match inner.nodes.get(&path) {
            Some(MemoryNode::Symlink { target, .. }) => Ok(target.clone()),
            Some(_) => Err(ErrorTrace::new(VfsError::NotAFile {
                path: format!("not a symlink: {path}"),
            })),
            None => Err(ErrorTrace::new(VfsError::NotFound { path })),
        }
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let from = normalize_path(from)?;
        let to = normalize_path(to)?;
        let mut inner = self.inner.write().unwrap();
        if !inner.nodes.contains_key(&from) {
            return Err(ErrorTrace::new(VfsError::NotFound { path: from }));
        }
        if inner.nodes.contains_key(&to) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: to }));
        }
        let from_prefix = format!("{from}/");
        let keys_to_move: Vec<String> = inner
            .nodes
            .keys()
            .filter(|k| *k == &from || k.starts_with(&from_prefix))
            .cloned()
            .collect();
        let version = inner.next_version();
        for key in keys_to_move {
            let node = inner.nodes.remove(&key).unwrap();
            let new_key = if key == from {
                to.clone()
            } else {
                format!("{to}{}", &key[from.len()..])
            };
            inner.nodes.insert(new_key, node);
        }
        if let Some(node) = inner.nodes.get_mut(&to) {
            node.metadata_mut().version = version;
        }
        Ok(())
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let path = normalize_path(path)?;
        if path == "/" {
            return Err(ErrorTrace::new(VfsError::PermissionDenied {
                path: "cannot remove root".to_string(),
            }));
        }
        let mut inner = self.inner.write().unwrap();
        let resolved = inner.resolve_symlinks(&path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::Directory { .. }) => {
                let children = inner.list_children(&resolved);
                if !children.is_empty() {
                    return Err(ErrorTrace::new(VfsError::NotAFile {
                        path: format!("directory not empty: {resolved}"),
                    }));
                }
            }
            Some(_) => {}
            None => {
                return Err(ErrorTrace::new(VfsError::NotFound { path: resolved }));
            }
        }
        if path != resolved {
            inner.nodes.remove(&path);
        }
        inner.nodes.remove(&resolved);
        inner.next_version();
        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let path = normalize_path(path)?;
        let inner = self.inner.read().unwrap();
        let resolved = inner.resolve_symlinks(&path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::File { content, .. }) => Ok(MemoryFile {
                content: content.clone(),
                path: resolved,
                fs: self.inner.clone(),
                mode,
            }),
            Some(_) => Err(ErrorTrace::new(VfsError::NotAFile { path: resolved })),
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = VfsFileSystem::open(self, path, mode)?;
        Ok(SeekableMemoryFile {
            inner: file,
            position: 0,
        })
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let path = normalize_path(path)?;
        let inner = self.inner.read().unwrap();
        let resolved = inner.resolve_symlinks(&path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::Directory { .. }) => Ok(MemoryDirectory {
                fs: self.inner.clone(),
                dir_path: resolved,
            }),
            Some(_) => Err(ErrorTrace::new(VfsError::NotADirectory { path: resolved })),
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let path = normalize_path(path)?;
        let mut inner = self.inner.write().unwrap();
        if inner.nodes.contains_key(&path) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: path.clone(),
            }));
        }
        if let Some(parent) = parent_path(&path) {
            if !inner.nodes.contains_key(&parent) {
                return Err(ErrorTrace::new(VfsError::NotFound { path: parent }));
            }
        }
        let version = inner.next_version();
        let content = Arc::new(RwLock::new(Vec::new()));
        let mut metadata = VfsMetadata::new_file(0, mode);
        metadata.version = version;
        inner.nodes.insert(
            path.clone(),
            MemoryNode::File {
                content: content.clone(),
                metadata,
            },
        );
        Ok(MemoryFile {
            content,
            path,
            fs: self.inner.clone(),
            mode: OpenMode::ReadWrite,
        })
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let path = normalize_path(path)?;
        let mut inner = self.inner.write().unwrap();
        if inner.nodes.contains_key(&path) {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: path.clone(),
            }));
        }
        if let Some(parent) = parent_path(&path) {
            if !inner.nodes.contains_key(&parent) {
                return Err(ErrorTrace::new(VfsError::NotFound { path: parent }));
            }
        }
        let version = inner.next_version();
        let mut metadata = VfsMetadata::new_directory(DEFAULT_DIR_PERMS);
        metadata.version = version;
        inner
            .nodes
            .insert(path, MemoryNode::Directory { metadata });
        Ok(())
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let path = normalize_path(path)?;
        let inner = self.inner.read().unwrap();
        let resolved = inner.resolve_symlinks(&path)?;
        match inner.nodes.get(&resolved) {
            Some(MemoryNode::File { content, .. }) => Ok(content.read().unwrap().clone()),
            Some(_) => Err(ErrorTrace::new(VfsError::NotAFile { path: resolved })),
            None => Err(ErrorTrace::new(VfsError::NotFound { path: resolved })),
        }
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let path = normalize_path(path)?;
        let mut inner = self.inner.write().unwrap();
        let version = inner.next_version();

        match inner.nodes.get(&path) {
            Some(MemoryNode::File { content, .. }) => {
                let mut content = content.write().unwrap();
                content.clear();
                content.extend_from_slice(data);
                drop(content);
                if let Some(node) = inner.nodes.get_mut(&path) {
                    let meta = node.metadata_mut();
                    meta.size = data.len() as u64;
                    meta.version = version;
                    meta.modified = Some(std::time::SystemTime::now());
                }
                Ok(())
            }
            Some(_) => Err(ErrorTrace::new(VfsError::NotAFile { path })),
            None => {
                if let Some(parent) = parent_path(&path) {
                    if !inner.nodes.contains_key(&parent) {
                        return Err(ErrorTrace::new(VfsError::NotFound { path: parent }));
                    }
                }
                let content = Arc::new(RwLock::new(data.to_vec()));
                let mut metadata = VfsMetadata::new_file(data.len() as u64, 0o644);
                metadata.version = version;
                inner.nodes.insert(
                    path,
                    MemoryNode::File {
                        content,
                        metadata,
                    },
                );
                Ok(())
            }
        }
    }
}

// ──────────────────────────────────────────────
// Async trait implementations (sync-native: no real awaiting)
// ──────────────────────────────────────────────

#[async_trait]
impl AsyncVfsFile for MemoryFile {
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
impl AsyncVfsFile for SeekableMemoryFile {
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
impl AsyncSeekableVfsFile for SeekableMemoryFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let mut buf = vec![0; len];
        let n = SeekableVfsFile::read(self, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        SeekableVfsFile::write(self, &data)
    }

    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        SeekableVfsFile::seek(self, pos)
    }

    fn position_async(&self) -> u64 {
        SeekableVfsFile::position(self)
    }
}

#[async_trait]
impl AsyncVfsDirectory for MemoryDirectory {
    type File = MemoryFile;
    type SeekableFile = SeekableMemoryFile;

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
        Ok(Box::new(AsyncMemoryDirectory::from_sync(dir)))
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
        Ok(Box::new(AsyncMemoryDirectory::from_sync(dir)))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        VfsDirectory::stat(self, &path)
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        VfsDirectory::exists(self, &path)
    }
}

/// Thin wrapper that makes a `Box<dyn VfsDirectory>` implement `AsyncVfsDirectory`.
/// Used when the sync-native `MemoryDirectory` creates child directories — the
/// returned `Box<dyn VfsDirectory>` must be re-wrapped as an async directory.
pub struct AsyncMemoryDirectory {
    inner: Box<dyn VfsDirectory<File = MemoryFile, SeekableFile = SeekableMemoryFile>>,
    cached_path: String,
}

impl AsyncMemoryDirectory {
    fn from_sync(dir: Box<dyn VfsDirectory<File = MemoryFile, SeekableFile = SeekableMemoryFile>>) -> Self {
        let cached_path = dir.path().to_string();
        Self { inner: dir, cached_path }
    }
}

#[async_trait]
impl AsyncVfsDirectory for AsyncMemoryDirectory {
    type File = MemoryFile;
    type SeekableFile = SeekableMemoryFile;

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
impl AsyncVfsFileSystem for MemoryFs {
    type File = MemoryFile;
    type SeekableFile = SeekableMemoryFile;
    type Directory = AsyncMemoryDirectory;

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
        Ok(AsyncMemoryDirectory::from_sync(Box::new(dir)))
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        VfsFileSystem::create(self, &path, mode)
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        VfsFileSystem::mkdir(self, &path)
    }
}
