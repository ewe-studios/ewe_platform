//! `DirectoryDelta`: a delta store in a host directory (the "shadow"
//! directory), behind the async VFS traits. A whiteout is a `.wh.<name>`
//! sentinel file next to the hidden entry.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use foundation_errstacks::ErrorTrace;

use crate::shared::vfs::async_traits::{AsyncDeltaStore, AsyncVfsDirectory, AsyncVfsFileSystem};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::path_utils::normalize_vfs_path as shared_normalize;
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

use super::native_fs::{NativeFile, NativeFs, SeekableNativeFile};

const WHITEOUT_PREFIX: &str = ".wh.";

pub struct DirectoryDelta {
    fs: NativeFs,
    whiteouts: RwLock<HashMap<String, u64>>,
}

impl DirectoryDelta {
    pub fn new(shadow_path: impl Into<PathBuf>) -> VfsResult<Self> {
        let shadow_path: PathBuf = shadow_path.into();
        if !shadow_path.exists() {
            std::fs::create_dir_all(&shadow_path)
                .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        }
        let fs = NativeFs::new(&shadow_path)?;
        let whiteouts = scan_whiteouts(&shadow_path);
        Ok(Self {
            fs,
            whiteouts: RwLock::new(whiteouts),
        })
    }

    pub fn shadow_root(&self) -> &Path {
        self.fs.root()
    }

    fn read_whiteouts(&self) -> VfsResult<RwLockReadGuard<'_, HashMap<String, u64>>> {
        self.whiteouts.read().map_err(|_| poisoned())
    }

    fn write_whiteouts(&self) -> VfsResult<RwLockWriteGuard<'_, HashMap<String, u64>>> {
        self.whiteouts.write().map_err(|_| poisoned())
    }
}

fn poisoned() -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::Backend {
        message: "whiteout table lock poisoned".into(),
    })
}

fn scan_whiteouts(root: &Path) -> HashMap<String, u64> {
    let mut whiteouts = HashMap::new();
    scan_whiteouts_recursive(root, root, &mut whiteouts);
    whiteouts
}

fn scan_whiteouts_recursive(root: &Path, dir: &Path, whiteouts: &mut HashMap<String, u64>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();

        if name.starts_with(WHITEOUT_PREFIX) {
            let basename = &name[WHITEOUT_PREFIX.len()..];
            let relative_dir = dir.strip_prefix(root).unwrap_or(Path::new(""));
            let vfs_path = if relative_dir == Path::new("") {
                format!("/{basename}")
            } else {
                format!("/{}/{basename}", relative_dir.to_string_lossy())
            };
            let version = std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            whiteouts.insert(vfs_path, version);
        }

        if path.is_dir() && !name.starts_with(WHITEOUT_PREFIX) {
            scan_whiteouts_recursive(root, &path, whiteouts);
        }
    }
}

fn normalize_vfs_path(path: &str) -> VfsResult<String> {
    shared_normalize(path)
}

fn whiteout_sentinel_path(shadow_root: &Path, vfs_path: &str) -> VfsResult<PathBuf> {
    let normalized = normalize_vfs_path(vfs_path)?;
    let (parent_part, basename) = match normalized.rfind('/') {
        Some(0) => ("", &normalized[1..]),
        Some(idx) => (&normalized[1..idx], &normalized[idx + 1..]),
        None => ("", normalized.as_str()),
    };
    let sentinel_name = format!("{WHITEOUT_PREFIX}{basename}");
    let path = if parent_part.is_empty() {
        shadow_root.join(sentinel_name)
    } else {
        shadow_root.join(parent_part).join(sentinel_name)
    };
    if !path.starts_with(shadow_root) {
        return Err(ErrorTrace::new(VfsError::PermissionDenied {
            path: format!("whiteout path escapes root: {vfs_path}"),
        }));
    }
    Ok(path)
}

#[async_trait]
impl AsyncDeltaStore for DirectoryDelta {
    async fn add_whiteout_async(&self, path: String, version: u64) -> VfsResult<()> {
        let sentinel = whiteout_sentinel_path(self.fs.root(), &path)?;
        if let Some(parent) = sentinel.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
            }
        }
        std::fs::write(&sentinel, version.to_string())
            .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        let mut whiteouts = self.write_whiteouts()?;
        whiteouts.insert(normalize_vfs_path(&path)?, version);
        Ok(())
    }

    async fn is_whiteout_async(&self, path: String) -> VfsResult<Option<u64>> {
        let whiteouts = self.read_whiteouts()?;
        let normalized = normalize_vfs_path(&path)?;
        let mut highest: Option<u64> = None;

        if let Some(&ver) = whiteouts.get(&normalized) {
            highest = Some(ver);
        }

        let mut current = normalized.clone();
        while let Some(idx) = current.rfind('/') {
            if idx == 0 {
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

    async fn remove_whiteout_async(&self, path: String) -> VfsResult<()> {
        let sentinel = whiteout_sentinel_path(self.fs.root(), &path)?;
        if sentinel.exists() {
            std::fs::remove_file(&sentinel)
                .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        }
        let mut whiteouts = self.write_whiteouts()?;
        whiteouts.remove(&normalize_vfs_path(&path)?);
        Ok(())
    }

    async fn list_whiteouts_async(&self, dir: String) -> VfsResult<Vec<(String, u64)>> {
        let whiteouts = self.read_whiteouts()?;
        let normalized_dir = normalize_vfs_path(&dir)?;
        let prefix = if normalized_dir == "/" {
            "/".to_string()
        } else {
            format!("{normalized_dir}/")
        };
        let mut result: Vec<(String, u64)> = whiteouts
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    async fn flush_async(&self) -> VfsResult<()> {
        Ok(())
    }

    async fn reset_async(&self) -> VfsResult<()> {
        let mut whiteouts = self.write_whiteouts()?;
        whiteouts.clear();
        drop(whiteouts);

        // Remove all contents of shadow directory
        let root = self.fs.root().to_path_buf();
        let entries =
            std::fs::read_dir(&root).map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                std::fs::remove_dir_all(&path)
                    .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
            } else {
                std::fs::remove_file(&path)
                    .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
            }
        }
        Ok(())
    }
}

type FilteredDirBox =
    Box<dyn AsyncVfsDirectory<File = NativeFile, SeekableFile = SeekableNativeFile>>;

#[async_trait]
impl AsyncVfsFileSystem for DirectoryDelta {
    type File = NativeFile;
    type SeekableFile = SeekableNativeFile;
    type Directory = FilteredDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        self.fs.capabilities()
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.fs.stat_async(path).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.fs.exists_async(path).await
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        self.fs.chmod_async(path, mode).await
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        self.fs.symlink_async(target, link).await
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        self.fs.readlink_async(path).await
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        self.fs.rename_async(from, to).await
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        self.fs.remove_async(path).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<NativeFile> {
        self.fs.open_async(path, mode).await
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<SeekableNativeFile> {
        self.fs.open_seekable_async(path, mode).await
    }

    /// Wraps the directory so that the `.wh.*` sentinel files stay hidden.
    async fn open_directory_async(&self, path: String) -> VfsResult<FilteredDirectory> {
        let inner = self.fs.open_directory_async(path).await?;
        Ok(FilteredDirectory {
            inner: Box::new(inner),
        })
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<NativeFile> {
        self.fs.create_async(path, mode).await
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        self.fs.mkdir_async(path).await
    }

    async fn inode_async(&self, path: String) -> VfsResult<u64> {
        self.fs.inode_async(path).await
    }

    async fn path_by_inode_async(&self, ino: u64) -> VfsResult<String> {
        self.fs.path_by_inode_async(ino).await
    }

    async fn stat_by_inode_async(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.fs.stat_by_inode_async(ino).await
    }

    async fn read_file_async(&self, path: String) -> VfsResult<Vec<u8>> {
        self.fs.read_file_async(path).await
    }

    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        self.fs.write_file_async(path, data).await
    }
}

// ── FilteredDirectory — hides .wh.* sentinel files ──

/// A shadow directory with the `.wh.*` sentinel files hidden. The
/// subdirectories it opens or creates are filtered too.
pub struct FilteredDirectory {
    inner: FilteredDirBox,
}

#[async_trait]
impl AsyncVfsDirectory for FilteredDirectory {
    type File = NativeFile;
    type SeekableFile = SeekableNativeFile;

    fn path(&self) -> String {
        self.inner.path()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata_async().await
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let entries = self.inner.list_async().await?;
        Ok(entries
            .into_iter()
            .filter(|e| !e.name.starts_with(WHITEOUT_PREFIX))
            .collect())
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        if name.starts_with(WHITEOUT_PREFIX) {
            return Ok(None);
        }
        self.inner.get_entry_async(name).await
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<NativeFile> {
        self.inner.create_file_async(name, mode).await
    }

    async fn create_dir_async(&self, name: String) -> VfsResult<FilteredDirBox> {
        let inner = self.inner.create_dir_async(name).await?;
        Ok(Box::new(FilteredDirectory { inner }))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        self.inner.remove_entry_async(name).await
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        self.inner.rename_entry_async(old_name, new_name).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<NativeFile> {
        self.inner.open_async(path, mode).await
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<SeekableNativeFile> {
        self.inner.open_seekable_async(path, mode).await
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<FilteredDirBox> {
        let inner = self.inner.open_directory_async(path).await?;
        Ok(Box::new(FilteredDirectory { inner }))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        self.inner.stat_async(path).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        self.inner.exists_async(path).await
    }
}
