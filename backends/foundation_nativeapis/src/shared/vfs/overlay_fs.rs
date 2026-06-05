use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_errstacks::ErrorTrace;

use super::error::{VfsError, VfsResult};
use super::path_utils::{normalize_vfs_path, parent_path};
use super::traits::{DeltaStore, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use super::types::{
    OpenMode, VfsCapabilities, VfsDirEntry, VfsFileType, VfsMetadata,
};

pub struct OverlayFileSystem<B: VfsFileSystem + 'static, D: DeltaStore + 'static> {
    base: B,
    delta: D,
    version: Arc<AtomicU64>,
}

impl<B: VfsFileSystem + 'static, D: DeltaStore + 'static> OverlayFileSystem<B, D> {
    pub fn new(base: B, delta: D) -> Self {
        Self {
            base,
            delta,
            version: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn base(&self) -> &B {
        &self.base
    }

    pub fn delta(&self) -> &D {
        &self.delta
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    fn next_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_visible(&self, path: &str) -> VfsResult<bool> {
        // Delta entry at exact path always wins — the overlay only creates delta
        // entries after handling whiteouts, so existence in delta means "created
        // after any deletion that produced the whiteout."
        if self.delta.exists(path)? {
            return Ok(true);
        }
        // No delta entry — check whiteouts
        if self.delta.is_whiteout(path)?.is_some() {
            return Ok(false);
        }
        // Not in delta, not whiteout'd — check base
        self.base.exists(path)
    }

    fn cow_to_delta(&self, path: &str) -> VfsResult<()> {
        let data = self.base.read_file(path)?;
        let base_meta = self.base.stat(path)?;

        // Ensure parent directories exist in delta
        if let Some(parent) = parent_path(path) {
            self.ensure_delta_parents(&parent)?;
        }

        let version = self.next_version();
        let file = self.delta.create(path, base_meta.permissions)?;
        if let Err(e) = file.write_at(&data, 0) {
            // Cleanup partial write
            let _ = self.delta.remove(path);
            return Err(e);
        }
        // The delta's create already gives us a version from the delta's internal counter,
        // but we want our overlay version. Update via chmod to trigger a version update.
        // Actually, we should just stamp the metadata. For now, the delta's own version
        // is sufficient — the overlay version is tracked separately in next_version().
        let _ = version;
        Ok(())
    }

    fn ensure_delta_parents(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let mut current = String::new();
        for part in parts {
            current = format!("{current}/{part}");
            if !self.delta.exists(&current)? {
                self.delta.mkdir(&current)?;
            }
        }
        Ok(())
    }
}

fn normalize_path(path: &str) -> VfsResult<String> {
    normalize_vfs_path(path)
}

// ── File handle types ──

pub enum OverlayFile<BF: VfsFile, DF: VfsFile> {
    Base(BF),
    Delta(DF),
}

impl<BF: VfsFile, DF: VfsFile> VfsFile for OverlayFile<BF, DF> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        match self {
            OverlayFile::Base(f) => f.read_at(buf, offset),
            OverlayFile::Delta(f) => f.read_at(buf, offset),
        }
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        match self {
            OverlayFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlayFile::Delta(f) => f.write_at(buf, offset),
        }
    }

    fn sync_data(&self) -> VfsResult<()> {
        match self {
            OverlayFile::Base(f) => f.sync_data(),
            OverlayFile::Delta(f) => f.sync_data(),
        }
    }

    fn size(&self) -> VfsResult<u64> {
        match self {
            OverlayFile::Base(f) => f.size(),
            OverlayFile::Delta(f) => f.size(),
        }
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        match self {
            OverlayFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlayFile::Delta(f) => f.truncate(size),
        }
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        match self {
            OverlayFile::Base(f) => f.metadata(),
            OverlayFile::Delta(f) => f.metadata(),
        }
    }
}

unsafe impl<BF: VfsFile, DF: VfsFile> Send for OverlayFile<BF, DF> {}
unsafe impl<BF: VfsFile, DF: VfsFile> Sync for OverlayFile<BF, DF> {}

pub enum OverlaySeekableFile<BF: SeekableVfsFile, DF: SeekableVfsFile> {
    Base(BF),
    Delta(DF),
}

impl<BF: SeekableVfsFile, DF: SeekableVfsFile> VfsFile for OverlaySeekableFile<BF, DF> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        match self {
            OverlaySeekableFile::Base(f) => f.read_at(buf, offset),
            OverlaySeekableFile::Delta(f) => f.read_at(buf, offset),
        }
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        match self {
            OverlaySeekableFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlaySeekableFile::Delta(f) => f.write_at(buf, offset),
        }
    }

    fn sync_data(&self) -> VfsResult<()> {
        match self {
            OverlaySeekableFile::Base(f) => f.sync_data(),
            OverlaySeekableFile::Delta(f) => f.sync_data(),
        }
    }

    fn size(&self) -> VfsResult<u64> {
        match self {
            OverlaySeekableFile::Base(f) => f.size(),
            OverlaySeekableFile::Delta(f) => f.size(),
        }
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        match self {
            OverlaySeekableFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlaySeekableFile::Delta(f) => f.truncate(size),
        }
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        match self {
            OverlaySeekableFile::Base(f) => f.metadata(),
            OverlaySeekableFile::Delta(f) => f.metadata(),
        }
    }
}

impl<BF: SeekableVfsFile, DF: SeekableVfsFile> SeekableVfsFile for OverlaySeekableFile<BF, DF> {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        match self {
            OverlaySeekableFile::Base(f) => f.read(buf),
            OverlaySeekableFile::Delta(f) => f.read(buf),
        }
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        match self {
            OverlaySeekableFile::Base(_) => Err(ErrorTrace::new(VfsError::ReadOnly)),
            OverlaySeekableFile::Delta(f) => f.write(buf),
        }
    }

    fn seek(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> {
        match self {
            OverlaySeekableFile::Base(f) => f.seek(pos),
            OverlaySeekableFile::Delta(f) => f.seek(pos),
        }
    }

    fn position(&self) -> u64 {
        match self {
            OverlaySeekableFile::Base(f) => f.position(),
            OverlaySeekableFile::Delta(f) => f.position(),
        }
    }
}

unsafe impl<BF: SeekableVfsFile, DF: SeekableVfsFile> Send for OverlaySeekableFile<BF, DF> {}
unsafe impl<BF: SeekableVfsFile, DF: SeekableVfsFile> Sync for OverlaySeekableFile<BF, DF> {}

// ── Overlay directory ──

pub struct OverlayDirectory<B: VfsFileSystem + 'static, D: DeltaStore + 'static> {
    base: *const B,
    delta: *const D,
    version: Arc<AtomicU64>,
    dir_path: String,
}

unsafe impl<B: VfsFileSystem + 'static, D: DeltaStore + 'static> Send for OverlayDirectory<B, D> {}
unsafe impl<B: VfsFileSystem + 'static, D: DeltaStore + 'static> Sync for OverlayDirectory<B, D> {}

impl<B: VfsFileSystem + 'static, D: DeltaStore + 'static> OverlayDirectory<B, D> {
    fn base(&self) -> &B {
        unsafe { &*self.base }
    }

    fn delta(&self) -> &D {
        unsafe { &*self.delta }
    }

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

    fn is_visible(&self, path: &str) -> VfsResult<bool> {
        if self.delta().exists(path)? {
            return Ok(true);
        }
        if self.delta().is_whiteout(path)?.is_some() {
            return Ok(false);
        }
        self.base().exists(path)
    }
}

impl<B: VfsFileSystem + 'static, D: DeltaStore + 'static> VfsDirectory for OverlayDirectory<B, D> {
    type File = OverlayFile<B::File, D::File>;
    type SeekableFile = OverlaySeekableFile<B::SeekableFile, D::SeekableFile>;

    fn path(&self) -> &str {
        &self.dir_path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        // Prefer delta metadata, fall back to base
        if self.delta().exists(&self.dir_path)? {
            return self.delta().stat(&self.dir_path);
        }
        self.base().stat(&self.dir_path)
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let mut entries = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        // 1. Collect delta entries
        if let Ok(delta_dir) = self.delta().open_directory(&self.dir_path) {
            if let Ok(delta_entries) = delta_dir.list() {
                for entry in delta_entries {
                    seen_names.insert(entry.name.clone());
                    entries.push(entry);
                }
            }
        }

        // 2. Collect base entries, filtering out whiteout'd and delta-shadowed
        if let Ok(base_dir) = self.base().open_directory(&self.dir_path) {
            if let Ok(base_entries) = base_dir.list() {
                for entry in base_entries {
                    if seen_names.contains(&entry.name) {
                        continue;
                    }
                    let child_path = if self.dir_path == "/" {
                        format!("/{}", entry.name)
                    } else {
                        format!("{}/{}", self.dir_path, entry.name)
                    };
                    if let Some(_wh_ver) = self.delta().is_whiteout(&child_path)? {
                        continue;
                    }
                    entries.push(entry);
                }
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let child_path = self.resolve_child_path(name)?;
        if !self.is_visible(&child_path)? {
            return Ok(None);
        }
        // Prefer delta
        if self.delta().exists(&child_path)? {
            let meta = self.delta().stat(&child_path)?;
            return Ok(Some(VfsDirEntry {
                name: name.to_string(),
                file_type: meta.file_type,
            }));
        }
        if self.base().exists(&child_path)? {
            let meta = self.base().stat(&child_path)?;
            return Ok(Some(VfsDirEntry {
                name: name.to_string(),
                file_type: meta.file_type,
            }));
        }
        Ok(None)
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let child_path = self.resolve_child_path(name)?;
        if self.is_visible(&child_path)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: child_path }));
        }
        let file = self.delta().create(&child_path, mode)?;
        Ok(OverlayFile::Delta(file))
    }

    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let child_path = self.resolve_child_path(name)?;
        if self.is_visible(&child_path)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: child_path }));
        }
        self.delta().mkdir(&child_path)?;
        Ok(Box::new(OverlayDirectory {
            base: self.base,
            delta: self.delta,
            version: self.version.clone(),
            dir_path: child_path,
        }))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let child_path = self.resolve_child_path(name)?;
        if !self.is_visible(&child_path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path: child_path }));
        }
        let in_delta = self.delta().exists(&child_path)?;
        let in_base = self.base().exists(&child_path)?;

        if in_delta {
            self.delta().remove(&child_path)?;
        }
        if in_base {
            let ver = self.version.fetch_add(1, Ordering::SeqCst) + 1;
            self.delta().add_whiteout(&child_path, ver)?;
        }
        Ok(())
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let old_path = self.resolve_child_path(old_name)?;
        let new_path = self.resolve_child_path(new_name)?;
        if !self.is_visible(&old_path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path: old_path }));
        }
        if self.is_visible(&new_path)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: new_path }));
        }
        // If source only in base, we need to CoW it to the new path
        let in_delta = self.delta().exists(&old_path)?;
        let in_base = self.base().exists(&old_path)?;

        if in_delta {
            self.delta().rename(&old_path, &new_path)?;
        } else if in_base {
            // CoW from base to delta at new path
            let data = self.base().read_file(&old_path)?;
            let meta = self.base().stat(&old_path)?;
            let file = self.delta().create(&new_path, meta.permissions)?;
            file.write_at(&data, 0)?;
        }

        if in_base {
            let ver = self.version.fetch_add(1, Ordering::SeqCst) + 1;
            self.delta().add_whiteout(&old_path, ver)?;
        }
        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let full_path = self.resolve_child_path(path)?;
        if !self.is_visible(&full_path)? {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_path.clone(),
            }));
        }
        match mode {
            OpenMode::Read => {
                if self.delta().exists(&full_path)? {
                    Ok(OverlayFile::Delta(self.delta().open(&full_path, mode)?))
                } else {
                    Ok(OverlayFile::Base(self.base().open(&full_path, mode)?))
                }
            }
            OpenMode::Write | OpenMode::ReadWrite => {
                if !self.delta().exists(&full_path)? {
                    // Need CoW
                    let data = self.base().read_file(&full_path)?;
                    let meta = self.base().stat(&full_path)?;
                    if let Some(parent) = parent_path(&full_path) {
                        let parts: Vec<&str> = parent.split('/').filter(|p| !p.is_empty()).collect();
                        let mut cur = String::new();
                        for part in parts {
                            cur = format!("{cur}/{part}");
                            if !self.delta().exists(&cur)? {
                                self.delta().mkdir(&cur)?;
                            }
                        }
                    }
                    let file = self.delta().create(&full_path, meta.permissions)?;
                    if let Err(e) = file.write_at(&data, 0) {
                        let _ = self.delta().remove(&full_path);
                        return Err(e);
                    }
                }
                Ok(OverlayFile::Delta(self.delta().open(&full_path, mode)?))
            }
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let full_path = self.resolve_child_path(path)?;
        if !self.is_visible(&full_path)? {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_path.clone(),
            }));
        }
        match mode {
            OpenMode::Read => {
                if self.delta().exists(&full_path)? {
                    Ok(OverlaySeekableFile::Delta(
                        self.delta().open_seekable(&full_path, mode)?,
                    ))
                } else {
                    Ok(OverlaySeekableFile::Base(
                        self.base().open_seekable(&full_path, mode)?,
                    ))
                }
            }
            OpenMode::Write | OpenMode::ReadWrite => {
                if !self.delta().exists(&full_path)? {
                    let data = self.base().read_file(&full_path)?;
                    let meta = self.base().stat(&full_path)?;
                    if let Some(parent) = parent_path(&full_path) {
                        let parts: Vec<&str> = parent.split('/').filter(|p| !p.is_empty()).collect();
                        let mut cur = String::new();
                        for part in parts {
                            cur = format!("{cur}/{part}");
                            if !self.delta().exists(&cur)? {
                                self.delta().mkdir(&cur)?;
                            }
                        }
                    }
                    let file = self.delta().create(&full_path, meta.permissions)?;
                    if let Err(e) = file.write_at(&data, 0) {
                        let _ = self.delta().remove(&full_path);
                        return Err(e);
                    }
                }
                Ok(OverlaySeekableFile::Delta(
                    self.delta().open_seekable(&full_path, mode)?,
                ))
            }
        }
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let full_path = self.resolve_child_path(path)?;
        if !self.is_visible(&full_path)? {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_path.clone(),
            }));
        }
        Ok(Box::new(OverlayDirectory {
            base: self.base,
            delta: self.delta,
            version: self.version.clone(),
            dir_path: full_path,
        }))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let full_path = self.resolve_child_path(path)?;
        if !self.is_visible(&full_path)? {
            return Err(ErrorTrace::new(VfsError::NotFound {
                path: full_path.clone(),
            }));
        }
        if self.delta().exists(&full_path)? {
            self.delta().stat(&full_path)
        } else {
            self.base().stat(&full_path)
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let full_path = self.resolve_child_path(path)?;
        self.is_visible(&full_path)
    }
}

// ── VfsFileSystem for OverlayFileSystem ──

impl<B: VfsFileSystem + 'static, D: DeltaStore + 'static> VfsFileSystem for OverlayFileSystem<B, D> {
    type File = OverlayFile<B::File, D::File>;
    type SeekableFile = OverlaySeekableFile<B::SeekableFile, D::SeekableFile>;
    type Directory = OverlayDirectory<B, D>;

    fn capabilities(&self) -> VfsCapabilities {
        let base_caps = self.base.capabilities();
        let delta_caps = self.delta.capabilities();
        VfsCapabilities {
            seekable: base_caps.seekable && delta_caps.seekable,
            symlinks: base_caps.symlinks && delta_caps.symlinks,
            permissions_enforced: false,
            event_emission: false,
            persistent: delta_caps.persistent,
        }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        if self.delta.exists(&path)? {
            self.delta.stat(&path)
        } else {
            self.base.stat(&path)
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let path = normalize_path(path)?;
        self.is_visible(&path)
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        if !self.delta.exists(&path)? {
            // CoW metadata change — copy to delta first
            let meta = self.base.stat(&path)?;
            if meta.file_type == VfsFileType::Directory {
                self.ensure_delta_parents(&path)?;
                if !self.delta.exists(&path)? {
                    self.delta.mkdir(&path)?;
                }
            } else {
                self.cow_to_delta(&path)?;
            }
        }
        self.delta.chmod(&path, mode)
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let link = normalize_path(link)?;
        if self.is_visible(&link)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: link }));
        }
        if let Some(parent) = parent_path(&link) {
            self.ensure_delta_parents(&parent)?;
        }
        self.delta.symlink(target, &link)
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let path = normalize_path(path)?;
        if self.delta.exists(&path)? {
            self.delta.readlink(&path)
        } else if self.base.exists(&path)? {
            self.base.readlink(&path)
        } else {
            Err(ErrorTrace::new(VfsError::NotFound { path }))
        }
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let from = normalize_path(from)?;
        let to = normalize_path(to)?;
        if !self.is_visible(&from)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path: from }));
        }
        if self.is_visible(&to)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path: to }));
        }
        let in_delta = self.delta.exists(&from)?;
        let in_base = self.base.exists(&from)?;

        if let Some(parent) = parent_path(&to) {
            self.ensure_delta_parents(&parent)?;
        }

        if in_delta {
            self.delta.rename(&from, &to)?;
        } else if in_base {
            let data = self.base.read_file(&from)?;
            let meta = self.base.stat(&from)?;
            let file = self.delta.create(&to, meta.permissions)?;
            file.write_at(&data, 0)?;
        }

        if in_base {
            let ver = self.next_version();
            self.delta.add_whiteout(&from, ver)?;
        }
        Ok(())
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        let in_delta = self.delta.exists(&path)?;
        let in_base = self.base.exists(&path)?;

        if in_delta {
            self.delta.remove(&path)?;
        }
        if in_base {
            let ver = self.next_version();
            self.delta.add_whiteout(&path, ver)?;
        }
        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        match mode {
            OpenMode::Read => {
                if self.delta.exists(&path)? {
                    Ok(OverlayFile::Delta(self.delta.open(&path, mode)?))
                } else {
                    Ok(OverlayFile::Base(self.base.open(&path, mode)?))
                }
            }
            OpenMode::Write | OpenMode::ReadWrite => {
                if !self.delta.exists(&path)? {
                    self.cow_to_delta(&path)?;
                }
                Ok(OverlayFile::Delta(self.delta.open(&path, mode)?))
            }
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        match mode {
            OpenMode::Read => {
                if self.delta.exists(&path)? {
                    Ok(OverlaySeekableFile::Delta(
                        self.delta.open_seekable(&path, mode)?,
                    ))
                } else {
                    Ok(OverlaySeekableFile::Base(
                        self.base.open_seekable(&path, mode)?,
                    ))
                }
            }
            OpenMode::Write | OpenMode::ReadWrite => {
                if !self.delta.exists(&path)? {
                    self.cow_to_delta(&path)?;
                }
                Ok(OverlaySeekableFile::Delta(
                    self.delta.open_seekable(&path, mode)?,
                ))
            }
        }
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        Ok(OverlayDirectory {
            base: &self.base as *const B,
            delta: &self.delta as *const D,
            version: self.version.clone(),
            dir_path: path,
        })
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let path = normalize_path(path)?;
        if self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path }));
        }
        if let Some(parent) = parent_path(&path) {
            self.ensure_delta_parents(&parent)?;
        }
        self.next_version();
        let file = self.delta.create(&path, mode)?;
        Ok(OverlayFile::Delta(file))
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let path = normalize_path(path)?;
        if self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::AlreadyExists { path }));
        }
        if let Some(parent) = parent_path(&path) {
            self.ensure_delta_parents(&parent)?;
        }
        self.next_version();
        self.delta.mkdir(&path)
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let path = normalize_path(path)?;
        if !self.is_visible(&path)? {
            return Err(ErrorTrace::new(VfsError::NotFound { path }));
        }
        if self.delta.exists(&path)? {
            self.delta.read_file(&path)
        } else {
            self.base.read_file(&path)
        }
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let path = normalize_path(path)?;
        self.next_version();
        if self.delta.exists(&path)? {
            self.delta.write_file(&path, data)
        } else if self.base.exists(&path)? {
            self.cow_to_delta(&path)?;
            self.delta.write_file(&path, data)
        } else {
            if let Some(parent) = parent_path(&path) {
                self.ensure_delta_parents(&parent)?;
            }
            self.delta.write_file(&path, data)
        }
    }
}
