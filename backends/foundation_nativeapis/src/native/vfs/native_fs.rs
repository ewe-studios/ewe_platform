use std::io::{Read as IoRead, Seek as IoSeek, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use foundation_errstacks::ErrorTrace;

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::path_utils::normalize_vfs_path as shared_normalize;
use crate::shared::vfs::traits::{SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use crate::shared::vfs::types::{
    Checksum, OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType,
    VfsMetadata,
};

#[cfg(unix)]
use std::os::unix::fs::{FileExt, MetadataExt};

#[derive(Clone)]
pub struct NativeFs {
    root: Arc<PathBuf>,
}

impl NativeFs {
    pub fn new(root: impl Into<PathBuf>) -> VfsResult<Self> {
        let root: PathBuf = root.into();
        let root = root.canonicalize().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        if !root.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotADirectory {
                path: root.display().to_string(),
            }));
        }
        Ok(Self { root: Arc::new(root) })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn resolve_path(&self, vfs_path: &str) -> VfsResult<PathBuf> {
        let vfs_path = normalize_vfs_path(vfs_path);
        if vfs_path == "/" {
            return Ok((*self.root).clone());
        }
        let relative = vfs_path.trim_start_matches('/');
        let joined = self.root.join(relative);

        // For existence-checking, we need to handle the case where the path
        // doesn't exist yet (create/mkdir). Try canonicalize; if it fails,
        // canonicalize the parent and append the last component.
        let resolved = if joined.exists() {
            joined.canonicalize().map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?
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
            let resolved_parent = parent.canonicalize().map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
            if !resolved_parent.starts_with(self.root.as_path()) {
                return Err(ErrorTrace::new(VfsError::PermissionDenied {
                    path: format!("path escapes root: {vfs_path}"),
                }));
            }
            resolved_parent.join(name)
        };

        if !resolved.starts_with(self.root.as_path()) {
            return Err(ErrorTrace::new(VfsError::PermissionDenied {
                path: format!("path escapes root: {vfs_path}"),
            }));
        }
        Ok(resolved)
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

fn normalize_vfs_path(path: &str) -> String {
    shared_normalize(path).unwrap_or_else(|_| "/".to_string())
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

    VfsMetadata {
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
            .map(|d| d.as_secs())
            .unwrap_or(0),
        state: VfsEntryState::Ready,
    }
}

// ── NativeFile ──

pub struct NativeFile {
    file: std::fs::File,
    path: PathBuf,
    mode: OpenMode,
}

impl VfsFile for NativeFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        #[cfg(unix)]
        {
            self.file.read_at(buf, offset).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
        #[cfg(not(unix))]
        {
            // Fallback: seek + read (not thread-safe for concurrent reads)
            use std::io::{Read, Seek};
            let file = &self.file;
            // This is inherently racy without pread, but works for single-threaded use
            let mut file_ref = file;
            // Can't easily do this without &mut on Windows; use a workaround
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "read_at on non-Unix".to_string(),
            }))
        }
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        #[cfg(unix)]
        {
            self.file.write_at(buf, offset).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
        #[cfg(not(unix))]
        {
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "write_at on non-Unix".to_string(),
            }))
        }
    }

    fn sync_data(&self) -> VfsResult<()> {
        self.file.sync_data().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn size(&self) -> VfsResult<u64> {
        let meta = self.file.metadata().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(meta.len())
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        self.file.set_len(size).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let meta = self.file.metadata().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(fs_metadata_to_vfs(&meta))
    }
}

unsafe impl Send for NativeFile {}
unsafe impl Sync for NativeFile {}

// ── SeekableNativeFile ──

pub struct SeekableNativeFile {
    file: RwLock<std::fs::File>,
    path: PathBuf,
    mode: OpenMode,
    position: std::sync::atomic::AtomicU64,
}

impl VfsFile for SeekableNativeFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        #[cfg(unix)]
        {
            let file = self.file.read().unwrap();
            file.read_at(buf, offset).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
        #[cfg(not(unix))]
        {
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "read_at on non-Unix".to_string(),
            }))
        }
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        #[cfg(unix)]
        {
            let file = self.file.read().unwrap();
            file.write_at(buf, offset).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
        #[cfg(not(unix))]
        {
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "write_at on non-Unix".to_string(),
            }))
        }
    }

    fn sync_data(&self) -> VfsResult<()> {
        let file = self.file.read().unwrap();
        file.sync_data().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn size(&self) -> VfsResult<u64> {
        let file = self.file.read().unwrap();
        let meta = file.metadata().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(meta.len())
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let file = self.file.read().unwrap();
        file.set_len(size).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let file = self.file.read().unwrap();
        let meta = file.metadata().map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(fs_metadata_to_vfs(&meta))
    }
}

impl SeekableVfsFile for SeekableNativeFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let mut file = self.file.write().unwrap();
        let n = IoRead::read(&mut *file, buf).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        self.position
            .fetch_add(n as u64, std::sync::atomic::Ordering::SeqCst);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        if self.mode == OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let mut file = self.file.write().unwrap();
        let n = IoWrite::write(&mut *file, buf).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        self.position
            .fetch_add(n as u64, std::sync::atomic::Ordering::SeqCst);
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let mut file = self.file.write().unwrap();
        let new_pos = IoSeek::seek(&mut *file, pos).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        self.position
            .store(new_pos, std::sync::atomic::Ordering::SeqCst);
        Ok(new_pos)
    }

    fn position(&self) -> u64 {
        self.position
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

unsafe impl Send for SeekableNativeFile {}
unsafe impl Sync for SeekableNativeFile {}

// ── NativeDirectory ──

pub struct NativeDirectory {
    fs_root: Arc<PathBuf>,
    dir_path: String,
}

impl NativeDirectory {
    fn resolve(&self, name: &str) -> VfsResult<PathBuf> {
        let vfs_path = if name == "." || name.is_empty() {
            self.dir_path.clone()
        } else if name.starts_with('/') {
            normalize_vfs_path(name)
        } else if self.dir_path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.dir_path)
        };
        let relative = vfs_path.trim_start_matches('/');
        let joined = self.fs_root.join(relative);
        Ok(joined)
    }

    fn child_vfs_path(&self, name: &str) -> String {
        if self.dir_path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.dir_path)
        }
    }
}

impl VfsDirectory for NativeDirectory {
    type File = NativeFile;
    type SeekableFile = SeekableNativeFile;

    fn path(&self) -> &str {
        &self.dir_path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let fs_path = self.resolve(".")?;
        let meta = std::fs::metadata(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(fs_metadata_to_vfs(&meta))
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let fs_path = self.resolve(".")?;
        let mut entries = Vec::new();
        let read_dir = std::fs::read_dir(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry.file_type().map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
            let file_type = if ft.is_dir() {
                VfsFileType::Directory
            } else if ft.is_symlink() {
                VfsFileType::Symlink
            } else {
                VfsFileType::Regular
            };
            entries.push(VfsDirEntry { name, file_type });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let fs_path = self.resolve(name)?;
        if !fs_path.exists() {
            return Ok(None);
        }
        let meta = std::fs::symlink_metadata(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        let file_type = if meta.is_dir() {
            VfsFileType::Directory
        } else if meta.is_symlink() {
            VfsFileType::Symlink
        } else {
            VfsFileType::Regular
        };
        Ok(Some(VfsDirEntry {
            name: name.to_string(),
            file_type,
        }))
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let fs_path = self.resolve(name)?;
        if fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: self.child_vfs_path(name),
            }));
        }
        let file = std::fs::File::create(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(&fs_path, perms).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
        }
        Ok(NativeFile {
            file,
            path: fs_path,
            mode: OpenMode::ReadWrite,
        })
    }

    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let fs_path = self.resolve(name)?;
        if fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: self.child_vfs_path(name),
            }));
        }
        std::fs::create_dir(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(Box::new(NativeDirectory {
            fs_root: self.fs_root.clone(),
            dir_path: self.child_vfs_path(name),
        }))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let fs_path = self.resolve(name)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: self.child_vfs_path(name),
            }));
        }
        if fs_path.is_dir() {
            std::fs::remove_dir(&fs_path).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
        } else {
            std::fs::remove_file(&fs_path).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
        }
        Ok(())
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let old_path = self.resolve(old_name)?;
        let new_path = self.resolve(new_name)?;
        std::fs::rename(&old_path, &new_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let full_vfs_path = self.child_vfs_path(path);
        let fs_path = self.resolve(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_vfs_path,
            }));
        }
        if fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotAFile {
                path: full_vfs_path,
            }));
        }
        let file = match mode {
            OpenMode::Read => std::fs::File::open(&fs_path),
            OpenMode::Write => std::fs::OpenOptions::new()
                .write(true)
                .open(&fs_path),
            OpenMode::ReadWrite => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&fs_path),
        }
        .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        Ok(NativeFile {
            file,
            path: fs_path,
            mode,
        })
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let full_vfs_path = self.child_vfs_path(path);
        let fs_path = self.resolve(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_vfs_path,
            }));
        }
        if fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotAFile {
                path: full_vfs_path,
            }));
        }
        let file = match mode {
            OpenMode::Read => std::fs::File::open(&fs_path),
            OpenMode::Write => std::fs::OpenOptions::new()
                .write(true)
                .open(&fs_path),
            OpenMode::ReadWrite => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&fs_path),
        }
        .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        Ok(SeekableNativeFile {
            file: RwLock::new(file),
            path: fs_path,
            mode,
            position: std::sync::atomic::AtomicU64::new(0),
        })
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let full_vfs_path = self.child_vfs_path(path);
        let fs_path = self.resolve(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_vfs_path.clone(),
            }));
        }
        if !fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotADirectory {
                path: full_vfs_path,
            }));
        }
        Ok(Box::new(NativeDirectory {
            fs_root: self.fs_root.clone(),
            dir_path: full_vfs_path,
        }))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let fs_path = self.resolve(path)?;
        let meta = std::fs::metadata(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(fs_metadata_to_vfs(&meta))
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        match self.resolve(path) {
            Ok(fs_path) => Ok(fs_path.exists()),
            Err(_) => Ok(false),
        }
    }
}

unsafe impl Send for NativeDirectory {}
unsafe impl Sync for NativeDirectory {}

// ── VfsFileSystem for NativeFs ──

impl VfsFileSystem for NativeFs {
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

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let fs_path = self.resolve_path(path)?;
        let meta = std::fs::metadata(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(fs_metadata_to_vfs(&meta))
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        match self.resolve_path(path) {
            Ok(fs_path) => Ok(fs_path.exists()),
            Err(_) => Ok(false),
        }
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let fs_path = self.resolve_path(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(&fs_path, perms).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (fs_path, mode);
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "chmod on non-Unix".to_string(),
            }))
        }
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let link_path = self.resolve_path(link)?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, &link_path).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (target, link_path);
            Err(ErrorTrace::new(VfsError::Unsupported {
                operation: "symlink on non-Unix".to_string(),
            }))
        }
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let fs_path = self.resolve_path(path)?;
        let target = std::fs::read_link(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        Ok(target.to_string_lossy().to_string())
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let from_path = self.resolve_path(from)?;
        let to_path = self.resolve_path(to)?;
        std::fs::rename(&from_path, &to_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let fs_path = self.resolve_path(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: path.to_string(),
            }));
        }
        if fs_path.is_dir() {
            std::fs::remove_dir(&fs_path).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        } else {
            std::fs::remove_file(&fs_path).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })
        }
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let fs_path = self.resolve_path(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: path.to_string(),
            }));
        }
        if fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotAFile {
                path: path.to_string(),
            }));
        }
        let file = match mode {
            OpenMode::Read => std::fs::File::open(&fs_path),
            OpenMode::Write => std::fs::OpenOptions::new().write(true).open(&fs_path),
            OpenMode::ReadWrite => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&fs_path),
        }
        .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        Ok(NativeFile {
            file,
            path: fs_path,
            mode,
        })
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let fs_path = self.resolve_path(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: path.to_string(),
            }));
        }
        if fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotAFile {
                path: path.to_string(),
            }));
        }
        let file = match mode {
            OpenMode::Read => std::fs::File::open(&fs_path),
            OpenMode::Write => std::fs::OpenOptions::new().write(true).open(&fs_path),
            OpenMode::ReadWrite => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&fs_path),
        }
        .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        Ok(SeekableNativeFile {
            file: RwLock::new(file),
            path: fs_path,
            mode,
            position: std::sync::atomic::AtomicU64::new(0),
        })
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let fs_path = self.resolve_path(path)?;
        if !fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: path.to_string(),
            }));
        }
        if !fs_path.is_dir() {
            return Err(ErrorTrace::new(VfsError::NotADirectory {
                path: path.to_string(),
            }));
        }
        let vfs_path = self.to_vfs_path(&fs_path);
        Ok(NativeDirectory {
            fs_root: self.root.clone(),
            dir_path: vfs_path,
        })
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let fs_path = self.resolve_path(path)?;
        if fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: path.to_string(),
            }));
        }
        let file = std::fs::File::create(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(&fs_path, perms).map_err(|e| {
                ErrorTrace::new(VfsError::Io { source: e })
            })?;
        }
        Ok(NativeFile {
            file,
            path: fs_path,
            mode: OpenMode::ReadWrite,
        })
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let fs_path = self.resolve_path(path)?;
        if fs_path.exists() {
            return Err(ErrorTrace::new(VfsError::AlreadyExists {
                path: path.to_string(),
            }));
        }
        std::fs::create_dir(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let fs_path = self.resolve_path(path)?;
        std::fs::read(&fs_path).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let fs_path = self.resolve_path(path)?;
        std::fs::write(&fs_path, data).map_err(|e| {
            ErrorTrace::new(VfsError::Io { source: e })
        })
    }
}
