use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use foundation_errstacks::ErrorTrace;

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::path_utils::normalize_vfs_path as shared_normalize;
use crate::shared::vfs::traits::{DeltaStore, VfsDirectory, VfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};
#[allow(unused_imports)]
use foundation_nostd::macros::scaffold;
use foundation_macros::scaffold_impl;

use super::native_fs::NativeFs;

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

impl DeltaStore for DirectoryDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let sentinel = whiteout_sentinel_path(self.fs.root(), path)?;
        if let Some(parent) = sentinel.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
            }
        }
        std::fs::write(&sentinel, version.to_string())
            .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        let mut whiteouts = self.whiteouts.write().unwrap();
        whiteouts.insert(normalize_vfs_path(path)?, version);
        Ok(())
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        let whiteouts = self.whiteouts.read().unwrap();
        let normalized = normalize_vfs_path(path)?;
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

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        let sentinel = whiteout_sentinel_path(self.fs.root(), path)?;
        if sentinel.exists() {
            std::fs::remove_file(&sentinel)
                .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        }
        let mut whiteouts = self.whiteouts.write().unwrap();
        whiteouts.remove(&normalize_vfs_path(path)?);
        Ok(())
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        let whiteouts = self.whiteouts.read().unwrap();
        let normalized_dir = normalize_vfs_path(dir)?;
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

    fn flush(&self) -> VfsResult<()> {
        Ok(())
    }

    fn reset(&self) -> VfsResult<()> {
        let mut whiteouts = self.whiteouts.write().unwrap();
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

#[scaffold_impl(via = "self.fs")]
impl VfsFileSystem for DirectoryDelta {
    type File = <NativeFs as VfsFileSystem>::File;
    type SeekableFile = <NativeFs as VfsFileSystem>::SeekableFile;
    type Directory = FilteredDirectory;

    // Override — wraps inner directory to filter out .wh.* sentinel files
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let inner = self.fs.open_directory(path)?;
        Ok(FilteredDirectory { inner })
    }

    // Delegated to self.fs
    fn capabilities(&self) -> VfsCapabilities { scaffold!() }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
    fn exists(&self, path: &str) -> VfsResult<bool> { scaffold!() }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { scaffold!() }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { scaffold!() }
    fn readlink(&self, path: &str) -> VfsResult<String> { scaffold!() }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> { scaffold!() }
    fn remove(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { scaffold!() }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { scaffold!() }
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> { scaffold!() }
    fn mkdir(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> { scaffold!() }
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> { scaffold!() }
}

// ── FilteredDirectory — hides .wh.* sentinel files from listings ──

pub struct FilteredDirectory {
    inner: <NativeFs as VfsFileSystem>::Directory,
}

#[scaffold_impl(via = "self.inner")]
impl VfsDirectory for FilteredDirectory {
    type File = <NativeFs as VfsFileSystem>::File;
    type SeekableFile = <NativeFs as VfsFileSystem>::SeekableFile;

    // Override — filter out .wh.* sentinel files from listings
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let entries = self.inner.list()?;
        Ok(entries
            .into_iter()
            .filter(|e| !e.name.starts_with(WHITEOUT_PREFIX))
            .collect())
    }

    // Override — block access to whiteout sentinel files
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        if name.starts_with(WHITEOUT_PREFIX) {
            return Ok(None);
        }
        self.inner.get_entry(name)
    }

    // Delegated to self.inner
    fn path(&self) -> &str { scaffold!() }
    fn metadata(&self) -> VfsResult<VfsMetadata> { scaffold!() }
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> { scaffold!() }
    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    { scaffold!() }
    fn remove_entry(&self, name: &str) -> VfsResult<()> { scaffold!() }
    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> { scaffold!() }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { scaffold!() }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { scaffold!() }
    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    { scaffold!() }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
    fn exists(&self, path: &str) -> VfsResult<bool> { scaffold!() }
}

unsafe impl Send for FilteredDirectory {}
unsafe impl Sync for FilteredDirectory {}
