use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::async_traits::{AsyncDeltaStore, AsyncVfsFileSystem};
use super::error::VfsResult;
use super::memory_fs::{MemoryFile, MemoryFs, SeekableMemoryFile};
use super::traits::{DeltaStore, VfsDirectory, VfsFileSystem};
use super::types::{OpenMode, VfsCapabilities, VfsMetadata};

pub struct MemoryDelta {
    fs: MemoryFs,
    whiteouts: RwLock<HashMap<String, u64>>,
}

impl MemoryDelta {
    pub fn new() -> Self {
        Self {
            fs: MemoryFs::new(),
            whiteouts: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryDelta {
    fn default() -> Self {
        Self::new()
    }
}

impl DeltaStore for MemoryDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let mut whiteouts = self.whiteouts.write().unwrap();
        whiteouts.insert(path.to_string(), version);
        Ok(())
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        let whiteouts = self.whiteouts.read().unwrap();

        let mut highest: Option<u64> = None;

        // Check exact path
        if let Some(&ver) = whiteouts.get(path) {
            highest = Some(ver);
        }

        // Check ancestor paths for inheritance
        let mut current = path.to_string();
        while let Some(idx) = current.rfind('/') {
            if idx == 0 {
                // Check root "/" whiteout
                if let Some(&ver) = whiteouts.get("/") {
                    highest = Some(highest.map_or(ver, |h: u64| h.max(ver)));
                }
                break;
            }
            current.truncate(idx);
            if let Some(&ver) = whiteouts.get(&current) {
                highest = Some(highest.map_or(ver, |h: u64| h.max(ver)));
            }
        }

        Ok(highest)
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        let mut whiteouts = self.whiteouts.write().unwrap();
        whiteouts.remove(path);
        Ok(())
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        let whiteouts = self.whiteouts.read().unwrap();
        let prefix = if dir == "/" {
            "/".to_string()
        } else {
            format!("{dir}/")
        };
        let mut result: Vec<(String, u64)> = whiteouts
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    fn flush(&self) -> VfsResult<()> {
        Ok(())
    }

    fn reset(&self) -> VfsResult<()> {
        // Clear whiteouts
        let mut whiteouts = self.whiteouts.write().unwrap();
        whiteouts.clear();
        drop(whiteouts);

        // Clear all files and dirs from the inner MemoryFs (except root)
        // We do this by getting the directory listing and removing everything
        let dir = self.fs.open_directory("/")?;
        let entries = dir.list()?;
        for entry in entries {
            let path = format!("/{}", entry.name);
            self.fs.remove_all(&path)?;
        }
        Ok(())
    }
}

impl VfsFileSystem for MemoryDelta {
    type File = <MemoryFs as VfsFileSystem>::File;
    type SeekableFile = <MemoryFs as VfsFileSystem>::SeekableFile;
    type Directory = <MemoryFs as VfsFileSystem>::Directory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsFileSystem::capabilities(&self.fs)
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        self.fs.stat(path)
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        self.fs.inode(path)
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.fs.path_by_inode(ino)
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.fs.stat_by_inode(ino)
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        self.fs.exists(path)
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        self.fs.chmod(path, mode)
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        self.fs.symlink(target, link)
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        self.fs.readlink(path)
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        self.fs.rename(from, to)
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        self.fs.remove(path)
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        self.fs.open(path, mode)
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        self.fs.open_seekable(path, mode)
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        self.fs.open_directory(path)
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        self.fs.create(path, mode)
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        self.fs.mkdir(path)
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        self.fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        self.fs.write_file(path, data)
    }
}

// ──────────────────────────────────────────────
// Async trait implementations (sync-native: delegate to sync)
// ──────────────────────────────────────────────

#[async_trait]
impl AsyncVfsFileSystem for MemoryDelta {
    type File = MemoryFile;
    type SeekableFile = SeekableMemoryFile;
    type Directory = <MemoryFs as AsyncVfsFileSystem>::Directory;

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
        <MemoryFs as AsyncVfsFileSystem>::open_directory_async(&self.fs, path).await
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        VfsFileSystem::create(self, &path, mode)
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        VfsFileSystem::mkdir(self, &path)
    }
}

#[async_trait]
impl AsyncDeltaStore for MemoryDelta {
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
