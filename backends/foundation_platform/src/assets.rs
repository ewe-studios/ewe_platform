//! `PlatformAssetManager` — the single authority for bundle resource I/O (F40).
//!
//! WHY: `resource_dir()` is a real filesystem path on desktop and iOS, but on
//! Android it is `asset://localhost/` — a content URI. `std::fs::read()`
//! returns `None` for every bundled asset there, which is why WASM app pages
//! answered "Not Found: index.html" on device: the files were inside the APK
//! and nothing ever took them out.
//!
//! WHAT: one non-generic manager that owns a VFS backend chosen at init time,
//! plus per-app version directories so each app can be updated, rolled back,
//! and pruned on its own schedule.
//!
//! HOW: on Android the backend is
//! `OverlayFileSystem<AssetResolverFs, DirectoryDelta>` — the APK is a lazy
//! read-only base, AppData is the writable delta. Nothing is extracted at
//! startup; a read resolves from the delta (OTA'd bytes, on disk) and falls
//! back to the base (APK, still compressed in place). On desktop and iOS the
//! backend is `NativeFs` over `resource_dir()` and version directories are
//! not used at all.
//!
//! All path traversal, copy-on-write, symlink, and whiteout handling comes
//! from the `foundation_nativeapis` VFS stack rather than being written here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use foundation_errstacks::ErrorTrace;
use foundation_nativeapis::shared::vfs::dynfs::DynFs;
use foundation_nativeapis::shared::vfs::error::{VfsError, VfsResult};
use foundation_nativeapis::shared::vfs::traits::{
    SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem,
};
use foundation_nativeapis::shared::vfs::types::{
    OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsFileType, VfsMetadata,
};
use tauri::{App, Manager, Runtime};
use tracing::{debug, info, warn};

use crate::manifest::{Manifest, ManifestFile, ManifestSource, MANIFEST_FILENAME};

/// Version directory prefix. Bare semver everywhere in the API; the `v` is
/// an internal naming convention so version directories are distinguishable
/// from anything else that might land in an app directory (L4).
const VERSION_PREFIX: &str = "v";

/// Largest single file an OTA manifest may declare (H5).
pub const MAX_OTA_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// Largest number of file entries one manifest may declare (M7).
pub const MAX_OTA_FILES: usize = 500;

/// How many versions of each app survive a prune: the active one and one to
/// roll back to.
pub const VERSIONS_KEPT: usize = 2;

/// Filename holding the last accepted manifest sequence (H8). Lives at the
/// base root, not in a version directory — it must outlive every version.
const SEQUENCE_FILE: &str = ".ewe_manifest_seq";

// ── Read-only VFS types over the APK bundle ─────────────────────────────

/// A file whose contents were fully resolved at open time.
///
/// WHY: `AssetResolver::get()` hands back an owned `Vec<u8>` — there is no
/// file descriptor into an APK entry to seek around in. Holding the bytes is
/// not a caching decision, it is the only shape the source offers.
pub struct ReadOnlyVfsFile {
    path: String,
    bytes: Vec<u8>,
    inode: u64,
}

impl std::fmt::Debug for ReadOnlyVfsFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadOnlyVfsFile")
            .field("path", &self.path)
            .field("size", &self.bytes.len())
            .finish()
    }
}

impl std::fmt::Display for ReadOnlyVfsFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} bytes, read-only)", self.path, self.bytes.len())
    }
}

impl ReadOnlyVfsFile {
    fn new(path: String, bytes: Vec<u8>) -> Self {
        let inode = stable_inode(&path);
        Self { path, bytes, inode }
    }
}

impl VfsFile for ReadOnlyVfsFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        if offset >= self.bytes.len() {
            return Ok(0);
        }
        let available = &self.bytes[offset..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        Ok(n)
    }

    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn sync_data(&self) -> VfsResult<()> {
        Ok(())
    }

    fn size(&self) -> VfsResult<u64> {
        Ok(self.bytes.len() as u64)
    }

    fn truncate(&self, _size: u64) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        Ok(VfsMetadata::new_file(
            self.inode,
            self.bytes.len() as u64,
            0o444,
        ))
    }
}

/// Seekable view over a [`ReadOnlyVfsFile`].
pub struct ReadOnlyVfsSeekableFile {
    inner: ReadOnlyVfsFile,
    cursor: u64,
}

impl std::fmt::Debug for ReadOnlyVfsSeekableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadOnlyVfsSeekableFile")
            .field("inner", &self.inner)
            .field("cursor", &self.cursor)
            .finish()
    }
}

impl std::fmt::Display for ReadOnlyVfsSeekableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @{}", self.inner, self.cursor)
    }
}

impl VfsFile for ReadOnlyVfsSeekableFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.inner.read_at(buf, offset)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.inner.write_at(buf, offset)
    }
    fn sync_data(&self) -> VfsResult<()> {
        self.inner.sync_data()
    }
    fn size(&self) -> VfsResult<u64> {
        self.inner.size()
    }
    fn truncate(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate(size)
    }
    fn metadata(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata()
    }
}

impl SeekableVfsFile for ReadOnlyVfsSeekableFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = self.inner.read_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn write(&mut self, _buf: &[u8]) -> VfsResult<usize> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let size = self.inner.bytes.len() as i64;
        let target = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => size + n,
            SeekFrom::Current(n) => self.cursor as i64 + n,
        };
        if target < 0 {
            return Err(ErrorTrace::new(VfsError::InvalidPath {
                path: format!("seek before start of {}", self.inner.path),
            }));
        }
        self.cursor = target as u64;
        Ok(self.cursor)
    }

    fn position(&self) -> u64 {
        self.cursor
    }
}

/// A directory view over a precomputed set of asset keys.
pub struct ReadOnlyVfsDirectory {
    path: String,
    entries: Vec<VfsDirEntry>,
    resolver: Arc<dyn AssetSource>,
}

impl std::fmt::Debug for ReadOnlyVfsDirectory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadOnlyVfsDirectory")
            .field("path", &self.path)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl std::fmt::Display for ReadOnlyVfsDirectory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} entries, read-only)", self.path, self.entries.len())
    }
}

impl VfsDirectory for ReadOnlyVfsDirectory {
    type File = ReadOnlyVfsFile;
    type SeekableFile = ReadOnlyVfsSeekableFile;

    fn path(&self) -> &str {
        &self.path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        Ok(VfsMetadata::new_directory(stable_inode(&self.path), 0o555))
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        Ok(self.entries.clone())
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        Ok(self.entries.iter().find(|e| e.name == name).cloned())
    }

    fn create_file(&self, _name: &str, _mode: u32) -> VfsResult<Self::File> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn create_dir(
        &self,
        _name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn remove_entry(&self, _name: &str) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn rename_entry(&self, _old_name: &str, _new_name: &str) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        if mode != OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let full = join_vfs(&self.path, path);
        let bytes = self.resolver.fetch(&full)?;
        Ok(ReadOnlyVfsFile::new(full, bytes))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        Ok(ReadOnlyVfsSeekableFile {
            inner: self.open(path, mode)?,
            cursor: 0,
        })
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        let full = join_vfs(&self.path, path);
        Ok(Box::new(ReadOnlyVfsDirectory {
            entries: self.resolver.children(&full),
            path: full,
            resolver: Arc::clone(&self.resolver),
        }))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let full = join_vfs(&self.path, path);
        self.resolver.stat(&full)
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        Ok(self.resolver.exists(&join_vfs(&self.path, path)))
    }
}

// ── AssetResolverFs ─────────────────────────────────────────────────────

/// The read side of a bundled asset store, decoupled from Tauri's generics.
///
/// WHY: `AssetResolver<R>` is generic over the runtime, but
/// [`PlatformAssetManager`] must not be — `PlatformSession` is non-generic
/// and owns one. This trait is the seam: `AssetResolverFs` erases `R` behind
/// it, and tests supply an in-memory source with no Tauri app at all.
pub trait AssetSource: Send + Sync {
    /// Every asset key in the bundle, normalized to a leading-slash VFS path.
    fn keys(&self) -> Vec<String>;

    /// The bytes for one asset, or `NotFound`.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::NotFound`] when the bundle has no such key.
    fn fetch(&self, path: &str) -> VfsResult<Vec<u8>>;

    /// Whether `path` names a file or a directory prefix in the bundle.
    fn exists(&self, path: &str) -> bool {
        let normalized = normalize(path);
        self.keys()
            .iter()
            .any(|k| *k == normalized || k.starts_with(&format!("{normalized}/")))
    }

    /// Metadata for a bundle entry.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::NotFound`] when the bundle has no such key.
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let normalized = normalize(path);
        if let Ok(bytes) = self.fetch(&normalized) {
            return Ok(VfsMetadata::new_file(
                stable_inode(&normalized),
                bytes.len() as u64,
                0o444,
            ));
        }
        if self.exists(&normalized) {
            return Ok(VfsMetadata::new_directory(
                stable_inode(&normalized),
                0o555,
            ));
        }
        Err(ErrorTrace::new(VfsError::NotFound { path: normalized }))
    }

    /// The immediate children of a directory prefix.
    fn children(&self, dir: &str) -> Vec<VfsDirEntry> {
        let dir = normalize(dir);
        let prefix = if dir == "/" {
            "/".to_string()
        } else {
            format!("{dir}/")
        };

        let mut seen: HashMap<String, VfsFileType> = HashMap::new();
        for key in self.keys() {
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            match rest.split_once('/') {
                // A deeper key implies an intermediate directory that no
                // asset key names on its own — the APK stores a flat list.
                Some((head, _)) => {
                    seen.insert(head.to_string(), VfsFileType::Directory);
                }
                None => {
                    seen.insert(rest.to_string(), VfsFileType::Regular);
                }
            }
        }

        let mut entries: Vec<VfsDirEntry> = seen
            .into_iter()
            .map(|(name, file_type)| VfsDirEntry {
                inode: stable_inode(&format!("{prefix}{name}")),
                name,
                file_type,
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }
}

/// A read-only `VfsFileSystem` over Tauri's embedded asset bundle.
///
/// Lazy by construction: nothing is read until a path is requested, and
/// nothing is ever written to disk. This is what makes "zero startup
/// extraction" true rather than aspirational.
pub struct AssetResolverFs {
    source: Arc<dyn AssetSource>,
}

impl std::fmt::Debug for AssetResolverFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetResolverFs")
            .field("keys", &self.source.keys().len())
            .finish()
    }
}

impl std::fmt::Display for AssetResolverFs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AssetResolverFs({} assets)", self.source.keys().len())
    }
}

impl AssetResolverFs {
    /// Wrap any [`AssetSource`].
    #[must_use]
    pub fn new(source: Arc<dyn AssetSource>) -> Self {
        Self { source }
    }

    /// Wrap a live Tauri [`tauri::AssetResolver`].
    #[must_use]
    pub fn from_resolver<R: Runtime>(resolver: tauri::AssetResolver<R>) -> Self {
        Self::new(Arc::new(TauriAssetSource { resolver }))
    }
}

/// [`AssetSource`] backed by Tauri's embedded bundle.
struct TauriAssetSource<R: Runtime> {
    resolver: tauri::AssetResolver<R>,
}

impl<R: Runtime> AssetSource for TauriAssetSource<R> {
    fn keys(&self) -> Vec<String> {
        self.resolver
            .iter()
            .map(|(key, _)| normalize(&key))
            .collect()
    }

    fn fetch(&self, path: &str) -> VfsResult<Vec<u8>> {
        let normalized = normalize(path);
        // Tauri keys assets without a leading slash; the VFS always has one.
        let key = normalized.trim_start_matches('/').to_string();
        self.resolver
            .get(key)
            .map(|asset| asset.bytes)
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: normalized }))
    }
}

impl VfsFileSystem for AssetResolverFs {
    type File = ReadOnlyVfsFile;
    type SeekableFile = ReadOnlyVfsSeekableFile;
    type Directory = ReadOnlyVfsDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities {
            seekable: true,
            symlinks: false,
            permissions_enforced: false,
            event_emission: false,
            // The bytes persist, but nothing written here would.
            persistent: false,
        }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        self.source.stat(&normalize_checked(path)?)
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        // A traversal attempt is "does not exist", not an error, so a caller
        // probing the overlay does not get a hard failure from the base.
        match normalize_checked(path) {
            Ok(p) => Ok(self.source.exists(&p)),
            Err(_) => Ok(false),
        }
    }

    fn chmod(&self, _path: &str, _mode: u32) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn symlink(&self, _target: &str, _link: &str) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        Err(ErrorTrace::new(VfsError::NotASymlink {
            path: path.to_string(),
        }))
    }

    fn rename(&self, _from: &str, _to: &str) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn remove(&self, _path: &str) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        if mode != OpenMode::Read {
            return Err(ErrorTrace::new(VfsError::ReadOnly));
        }
        let normalized = normalize_checked(path)?;
        let bytes = self.source.fetch(&normalized)?;
        Ok(ReadOnlyVfsFile::new(normalized, bytes))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        Ok(ReadOnlyVfsSeekableFile {
            inner: self.open(path, mode)?,
            cursor: 0,
        })
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let normalized = normalize_checked(path)?;
        if normalized != "/" && !self.source.exists(&normalized) {
            return Err(ErrorTrace::new(VfsError::NotFound { path: normalized }));
        }
        Ok(ReadOnlyVfsDirectory {
            entries: self.source.children(&normalized),
            path: normalized,
            resolver: Arc::clone(&self.source),
        })
    }

    fn create(&self, _path: &str, _mode: u32) -> VfsResult<Self::File> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn mkdir(&self, _path: &str) -> VfsResult<()> {
        Err(ErrorTrace::new(VfsError::ReadOnly))
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        Ok(stable_inode(&normalize_checked(path)?))
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.source
            .keys()
            .into_iter()
            .find(|k| stable_inode(k) == ino)
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::NotFound {
                    path: format!("inode {ino}"),
                })
            })
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        let path = self.path_by_inode(ino)?;
        self.source.stat(&path)
    }
}

// ── Path helpers ────────────────────────────────────────────────────────

/// Normalize to a leading-slash, no-trailing-slash VFS path. Traversal
/// components are dropped rather than rejected — use [`normalize_checked`]
/// where rejection is the correct answer.
fn normalize(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 1);
    out.push('/');
    for part in path.split('/').filter(|p| !p.is_empty() && *p != ".") {
        if part == ".." {
            continue;
        }
        if out.len() > 1 {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

/// Normalize, rejecting anything that tries to escape the root or smuggle a
/// separator past it (C1).
fn normalize_checked(path: &str) -> VfsResult<String> {
    if path.contains('\0') || path.contains('\\') {
        return Err(ErrorTrace::new(VfsError::InvalidPath {
            path: path.to_string(),
        }));
    }
    if path.split('/').any(|p| p == "..") {
        return Err(ErrorTrace::new(VfsError::InvalidPath {
            path: format!("path traversal not allowed: {path}"),
        }));
    }
    Ok(normalize(path))
}

fn join_vfs(base: &str, child: &str) -> String {
    if child == "." || child.is_empty() {
        return normalize(base);
    }
    normalize(&format!("{base}/{child}"))
}

/// A deterministic inode for a path. The VFS wants stable numbers for
/// equality and reverse lookup; the APK has no real inodes to report.
fn stable_inode(path: &str) -> u64 {
    // FNV-1a: cheap, deterministic, and adequate for identity within one
    // bundle. Nothing security-relevant depends on it.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Zero is reserved by several VFS consumers as "no inode".
    hash | 1
}

// ── Semver ──────────────────────────────────────────────────────────────

/// Parse a version into a comparable tuple.
///
/// WHY (C4): version directories sort lexically by default, which puts
/// `v0.10.0` before `v0.2.0` and prunes the newest release. Comparison must
/// be numeric per component.
///
/// Accepts an optional `v` prefix so callers can pass either a bare version
/// or a directory name.
#[must_use]
pub fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.strip_prefix(VERSION_PREFIX).unwrap_or(version);
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Reject version strings that could escape a directory or confuse the
/// filesystem (M1). A valid version is a charset-restricted string that also
/// parses as semver.
fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty() {
        return Err("version is empty".to_string());
    }
    if !version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    {
        return Err(format!("version has illegal characters: {version:?}"));
    }
    if parse_semver(version).is_none() {
        return Err(format!("version is not semver: {version:?}"));
    }
    Ok(())
}

/// Reject app ids that could escape their directory (C1).
fn validate_app_id(app_id: &str) -> Result<(), String> {
    if app_id.is_empty() {
        return Err("app_id is empty".to_string());
    }
    if app_id.contains('/')
        || app_id.contains('\\')
        || app_id.contains('\0')
        || app_id.contains("..")
    {
        return Err(format!("app_id has illegal characters: {app_id:?}"));
    }
    Ok(())
}

/// Reject file paths inside a bundle (C1).
fn validate_file_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("file path is empty".to_string());
    }
    if path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err(format!("file path is not relative: {path:?}"));
    }
    if path.split('/').any(|p| p == "..") {
        return Err(format!("file path escapes its bundle: {path:?}"));
    }
    Ok(())
}

// ── Layout ──────────────────────────────────────────────────────────────

/// Whether this platform keeps per-app version directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display)]
pub enum AssetLayout {
    /// Desktop and iOS: `resource_dir()` is already the app's real directory
    /// and the OS package manager owns its lifecycle. No version directories,
    /// no activation, no pruning.
    #[display("Flat")]
    Flat,

    /// Android: `{app_id}/v{version}/` under AppData, one lifecycle per app.
    #[display("Versioned")]
    Versioned,
}

// ── OTA plan ────────────────────────────────────────────────────────────

/// One file an accepted manifest says we should fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaDownload {
    /// Which app the file belongs to.
    pub app_id: String,
    /// The version directory it lands in.
    pub bundle_version: String,
    /// Declared path, hash, and size — the hash is checked after download.
    pub file: ManifestFile,
    /// Fully derived URL. Never taken verbatim from the manifest (C2).
    pub url: String,
    /// Destination inside the VFS, relative to `base_root`.
    pub vfs_path: String,
}

impl std::fmt::Display for OtaDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} → {}", self.url, self.vfs_path)
    }
}

/// The work an accepted manifest implies.
#[derive(Debug, Clone)]
pub struct OtaPlan {
    /// The verified manifest, kept so it can be written into the version
    /// directory as provenance once every download succeeds.
    pub manifest: Manifest,
    /// Files to fetch, in manifest order.
    pub downloads: Vec<OtaDownload>,
}

impl std::fmt::Display for OtaPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OtaPlan(seq={}, {} downloads, rollback_to={:?})",
            self.manifest.sequence,
            self.downloads.len(),
            self.manifest.rollback_to
        )
    }
}

// ── PlatformAssetManager ────────────────────────────────────────────────

/// Cross-platform bundle resource manager backed by the VFS stack.
///
/// Non-generic on purpose: `PlatformSession` owns one and is itself
/// non-generic over Tauri's `Runtime`.
pub struct PlatformAssetManager {
    /// The VFS backend. All I/O goes through this — never `std::fs` directly.
    fs: DynFs,
    /// Directory holding the per-app directories. Never changes: version
    /// directories live inside the VFS namespace, not in the mount point,
    /// so one overlay covers every app and every version.
    base_root: PathBuf,
    /// The version the running binary shipped with.
    bundle_version: String,
    /// Per-app active version. Defaults to `bundle_version` for every app;
    /// `activate()` overrides one entry for the life of the session.
    active: RwLock<HashMap<String, String>>,
    /// Whether version directories are in play.
    layout: AssetLayout,
    /// Baked CDN domain. `None` disables OTA entirely.
    manifest_domain: Option<String>,
    /// Baked Ed25519 public key. `None` accepts unsigned manifests, which is
    /// only appropriate for local development.
    manifest_key: Option<[u8; 32]>,
    /// Last accepted manifest sequence (H8), mirrored to disk.
    last_manifest_seq: RwLock<u64>,
    /// Serializes activate / delete / prune / manifest processing (C5).
    /// Plain reads never take it.
    ops_lock: Mutex<()>,
}

impl std::fmt::Debug for PlatformAssetManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformAssetManager")
            .field("base_root", &self.base_root)
            .field("bundle_version", &self.bundle_version)
            .field("layout", &self.layout)
            .field("manifest_domain", &self.manifest_domain)
            .field("signed", &self.manifest_key.is_some())
            .finish()
    }
}

impl std::fmt::Display for PlatformAssetManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlatformAssetManager({} layout={} version={})",
            self.base_root.display(),
            self.layout,
            self.bundle_version
        )
    }
}

impl PlatformAssetManager {
    /// Build a manager over an explicit VFS backend.
    ///
    /// This is the constructor the test suites use: a `MemoryFs` or a
    /// `NativeFs` over a temp directory gives full version-management
    /// coverage on the host, with no Android device and no Tauri `App`.
    #[must_use]
    pub fn from_vfs(
        fs: DynFs,
        base_root: PathBuf,
        bundle_version: &str,
        layout: AssetLayout,
        manifest_domain: Option<String>,
        manifest_key: Option<[u8; 32]>,
    ) -> Self {
        let last_manifest_seq = read_sequence(&fs);
        Self {
            fs,
            base_root,
            bundle_version: bundle_version.to_string(),
            active: RwLock::new(HashMap::new()),
            layout,
            manifest_domain,
            manifest_key,
            last_manifest_seq: RwLock::new(last_manifest_seq),
            ops_lock: Mutex::new(()),
        }
    }

    /// Initialize from a live Tauri app.
    ///
    /// `bundle_version` comes from `app.package_info().version` — semver
    /// validated by Tauri's own config schema, and always increasing because
    /// that is what a release is.
    ///
    /// Android gets `OverlayFileSystem<AssetResolverFs, DirectoryDelta>` and
    /// `AssetLayout::Versioned`; every other platform gets `NativeFs` over
    /// `resource_dir()` and `AssetLayout::Flat`.
    ///
    /// # Panics
    ///
    /// Never panics. A backend that cannot be constructed falls back to an
    /// empty in-memory filesystem so the app still starts and every read
    /// reports `NotFound` rather than the process dying at launch.
    pub fn initialize<R: Runtime>(
        app: &App<R>,
        bundle_version: &str,
        manifest_domain: Option<String>,
        manifest_key: Option<[u8; 32]>,
    ) -> Self {
        let (fs, base_root, layout) = build_platform_vfs(app);

        let manager = Self::from_vfs(
            fs,
            base_root,
            bundle_version,
            layout,
            manifest_domain,
            manifest_key,
        );

        if layout == AssetLayout::Versioned {
            manager.ensure_version_dirs();
            manager.prune_all(VERSIONS_KEPT);
        }

        info!(
            base = %manager.base_root.display(),
            layout = %layout,
            version = bundle_version,
            signed = manager.manifest_key.is_some(),
            "asset manager ready"
        );
        manager
    }

    // ── Accessors ──

    /// The directory holding the per-app directories.
    #[must_use]
    pub fn base_root(&self) -> &Path {
        &self.base_root
    }

    /// The version the running binary shipped with.
    #[must_use]
    pub fn bundle_version(&self) -> &str {
        &self.bundle_version
    }

    /// Whether version directories are in use on this platform.
    #[must_use]
    pub fn layout(&self) -> AssetLayout {
        self.layout
    }

    /// The CDN domain baked into this build, if OTA is enabled.
    #[must_use]
    pub fn manifest_domain(&self) -> Option<&str> {
        self.manifest_domain.as_deref()
    }

    /// Whether this build can authenticate OTA manifests.
    #[must_use]
    pub fn has_manifest_key(&self) -> bool {
        self.manifest_key.is_some()
    }

    /// The active version for one app. Defaults to [`Self::bundle_version`]
    /// — a freshly installed binary serves what it shipped with.
    #[must_use]
    pub fn active_version(&self, app_id: &str) -> String {
        self.active
            .read()
            .ok()
            .and_then(|map| map.get(app_id).cloned())
            .unwrap_or_else(|| self.bundle_version.clone())
    }

    /// The real filesystem directory an app's assets are served from.
    ///
    /// `MobileDirectory` responders mount here. On Android this is the delta
    /// directory: OTA'd files are present, APK-bundled ones are not, which is
    /// why [`MobileApp`](crate::MobileApp) reads through
    /// [`Self::read_app_file`] rather than `std::fs` when a manager exists.
    #[must_use]
    pub fn app_root(&self, app_id: &str) -> PathBuf {
        match self.layout {
            AssetLayout::Flat => self.base_root.join(app_id),
            AssetLayout::Versioned => self
                .base_root
                .join(app_id)
                .join(format!("{VERSION_PREFIX}{}", self.active_version(app_id))),
        }
    }

    /// The VFS path of an app's active directory, relative to `base_root`.
    #[must_use]
    pub fn app_vfs_root(&self, app_id: &str) -> String {
        match self.layout {
            AssetLayout::Flat => format!("/{app_id}"),
            AssetLayout::Versioned => self.version_vfs_path(app_id, &self.active_version(app_id)),
        }
    }

    /// The VFS path of a specific version directory.
    #[must_use]
    pub fn version_vfs_path(&self, app_id: &str, version: &str) -> String {
        match self.layout {
            AssetLayout::Flat => format!("/{app_id}"),
            AssetLayout::Versioned => format!("/{app_id}/{VERSION_PREFIX}{version}"),
        }
    }

    // ── I/O ──

    /// Read a file. `path` is relative to `base_root`.
    ///
    /// On Android this resolves from the delta (OTA'd bytes on disk) first,
    /// then the base (APK), then reports `NotFound`.
    ///
    /// # Errors
    ///
    /// Propagates any [`VfsError`] from the backend, including
    /// [`VfsError::InvalidPath`] for a traversal attempt.
    pub fn read(&self, path: &str) -> VfsResult<Vec<u8>> {
        self.fs.read_file(&normalize_checked(path)?)
    }

    /// Read a file belonging to one app, resolved against its active version.
    ///
    /// # Errors
    ///
    /// Propagates any [`VfsError`] from the backend.
    pub fn read_app_file(&self, app_id: &str, relative: &str) -> VfsResult<Vec<u8>> {
        let root = self.app_vfs_root(app_id);
        self.read(&format!("{root}/{relative}"))
    }

    /// Write a file. `path` is relative to `base_root`. Parent directories
    /// are created. On Android the bytes land in the delta layer.
    ///
    /// # Errors
    ///
    /// Propagates any [`VfsError`] from the backend, including
    /// [`VfsError::ReadOnly`] if the backend has no writable layer.
    pub fn write(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let normalized = normalize_checked(path)?;
        if let Some(parent) = normalized.rsplit_once('/').map(|(p, _)| p) {
            if !parent.is_empty() {
                self.fs.mkdir_all(parent)?;
            }
        }
        self.fs.write_file(&normalized, data)
    }

    /// Whether a path exists in either overlay layer.
    ///
    /// # Errors
    ///
    /// Propagates any [`VfsError`] from the backend.
    pub fn exists(&self, path: &str) -> VfsResult<bool> {
        self.fs.exists(&normalize_checked(path)?)
    }

    /// Move a path. Both sides are relative to `base_root`.
    ///
    /// This is the commit step of a staged write: a `.part` file becomes the
    /// real one only once its bytes have been verified.
    ///
    /// # Errors
    ///
    /// Propagates any [`VfsError`] from the backend.
    pub fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        self.fs
            .rename(&normalize_checked(from)?, &normalize_checked(to)?)
    }

    /// Delete a path. Relative to `base_root`.
    ///
    /// # Errors
    ///
    /// Propagates any [`VfsError`] from the backend.
    pub fn remove(&self, path: &str) -> VfsResult<()> {
        self.fs.remove(&normalize_checked(path)?)
    }

    // ── Version management ──

    /// Every app directory under `base_root`, sorted.
    #[must_use]
    pub fn list_apps(&self) -> Vec<String> {
        let Ok(dir) = self.fs.open_dir("/") else {
            return Vec::new();
        };
        let Ok(entries) = dir.list() else {
            return Vec::new();
        };
        let mut apps: Vec<String> = entries
            .into_iter()
            .filter(|e| e.file_type == VfsFileType::Directory && !e.name.starts_with('.'))
            .map(|e| e.name)
            .collect();
        apps.sort();
        apps
    }

    /// One app's version directories, semver-sorted, newest last.
    ///
    /// Returns bare version strings — the `v` prefix is an internal naming
    /// convention (L4). Always empty under [`AssetLayout::Flat`].
    #[must_use]
    pub fn list_versions(&self, app_id: &str) -> Vec<String> {
        if self.layout == AssetLayout::Flat || validate_app_id(app_id).is_err() {
            return Vec::new();
        }
        let Ok(dir) = self.fs.open_dir(&format!("/{app_id}")) else {
            return Vec::new();
        };
        let Ok(entries) = dir.list() else {
            return Vec::new();
        };

        let mut versions: Vec<(u64, u64, u64, String)> = entries
            .into_iter()
            .filter(|e| e.file_type == VfsFileType::Directory)
            .filter_map(|e| {
                let bare = e.name.strip_prefix(VERSION_PREFIX)?;
                let (major, minor, patch) = parse_semver(bare)?;
                Some((major, minor, patch, bare.to_string()))
            })
            .collect();

        // Numeric per component (C4) — lexical order puts v0.10.0 before
        // v0.2.0 and would prune the newest release.
        versions.sort();
        versions.into_iter().map(|(_, _, _, v)| v).collect()
    }

    /// Point one app at a different version. Other apps are untouched.
    ///
    /// # Errors
    ///
    /// Returns an error string if the app id or version is malformed, or if
    /// the version directory does not exist.
    pub fn activate(&self, app_id: &str, version: &str) -> Result<(), String> {
        validate_app_id(app_id)?;
        validate_version(version)?;

        if self.layout == AssetLayout::Flat {
            debug!(app = app_id, version, "activate is a no-op under Flat layout");
            return Ok(());
        }

        let _guard = self.ops_lock.lock().map_err(|e| e.to_string())?;

        let path = self.version_vfs_path(app_id, version);
        match self.fs.exists(&path) {
            Ok(true) => {}
            Ok(false) => return Err(format!("version directory does not exist: {path}")),
            Err(e) => return Err(format!("checking {path}: {e}")),
        }

        self.active
            .write()
            .map_err(|e| e.to_string())?
            .insert(app_id.to_string(), version.to_string());

        info!(app = app_id, version, "activated version");
        Ok(())
    }

    /// Delete one app's version directory, with the manifest inside it.
    ///
    /// Refuses to delete the active version, the rollback target, or the last
    /// remaining version (H3). A version that does not exist is a warning,
    /// not an error — an OTA carrying a stale `delete_after` should still
    /// apply the rest of its directives (M5).
    ///
    /// # Errors
    ///
    /// Returns an error string when the deletion is refused or the underlying
    /// removal fails.
    pub fn delete_version(
        &self,
        app_id: &str,
        version: &str,
        rollback_target: Option<&str>,
    ) -> Result<(), String> {
        validate_app_id(app_id)?;
        validate_version(version)?;

        if self.layout == AssetLayout::Flat {
            return Ok(());
        }

        let _guard = self.ops_lock.lock().map_err(|e| e.to_string())?;

        if version == self.active_version(app_id) {
            return Err(format!(
                "refusing to delete the active version of {app_id}: {version}"
            ));
        }
        if rollback_target == Some(version) {
            return Err(format!(
                "refusing to delete the rollback target of {app_id}: {version}"
            ));
        }

        let versions = self.list_versions(app_id);
        if !versions.iter().any(|v| v == version) {
            warn!(
                app = app_id,
                version, "delete_after names a version that does not exist — ignoring"
            );
            return Ok(());
        }
        if versions.len() <= 1 {
            return Err(format!(
                "refusing to delete the only version of {app_id}: {version}"
            ));
        }

        let path = self.version_vfs_path(app_id, version);
        self.fs
            .remove_all(&path)
            .map_err(|e| format!("removing {path}: {e}"))?;

        info!(app = app_id, version, "deleted version directory");
        Ok(())
    }

    /// Keep the newest `keep` versions of one app, delete the rest.
    ///
    /// The active version is never a deletion candidate no matter how old it
    /// is, but it does count toward `keep`.
    ///
    /// # Errors
    ///
    /// Returns an error string if a removal fails.
    pub fn prune(&self, app_id: &str, keep: usize) -> Result<(), String> {
        if self.layout == AssetLayout::Flat || keep == 0 {
            return Ok(());
        }
        validate_app_id(app_id)?;

        let _guard = self.ops_lock.lock().map_err(|e| e.to_string())?;

        let active = self.active_version(app_id);
        let versions = self.list_versions(app_id);
        let mut remaining = versions.len();

        // Oldest first, active filtered out but still counted.
        let mut candidates = versions.iter().filter(|v| **v != active);
        while remaining > keep {
            let Some(victim) = candidates.next() else {
                break;
            };
            let path = self.version_vfs_path(app_id, victim);
            // remove_all takes the whole version directory, its manifest
            // included — no orphaned .ewe_manifest.json can outlive its
            // version.
            self.fs
                .remove_all(&path)
                .map_err(|e| format!("pruning {path}: {e}"))?;
            remaining -= 1;
            info!(app = app_id, version = %victim, "pruned old version");
        }
        Ok(())
    }

    /// Prune every app.
    pub fn prune_all(&self, keep: usize) {
        for app_id in self.list_apps() {
            if let Err(e) = self.prune(&app_id, keep) {
                warn!(app = %app_id, error = %e, "prune failed");
            }
        }
    }

    /// Create this launch's version directory for every app that has one.
    ///
    /// Idempotent by construction: `mkdir_all` on an existing directory is a
    /// no-op, and the directory is only the *delta* — it stays empty until an
    /// OTA writes into it, because the APK's own copy is served from the base
    /// layer.
    fn ensure_version_dirs(&self) {
        for app_id in self.list_apps() {
            let path = self.version_vfs_path(app_id.as_str(), &self.bundle_version);
            if let Err(e) = self.fs.mkdir_all(&path) {
                warn!(app = %app_id, path = %path, error = %e, "could not create version directory");
            }
        }
    }

    // ── Manifest processing ──

    /// The last manifest sequence this device accepted.
    #[must_use]
    pub fn last_manifest_sequence(&self) -> u64 {
        self.last_manifest_seq.read().map_or(0, |v| *v)
    }

    /// Verify an OTA manifest and turn it into a download plan.
    ///
    /// In order: signature, schema, domain, replay, then per-entry limits and
    /// path validation. Nothing is fetched and nothing is written — this
    /// function only decides whether the manifest is worth acting on.
    ///
    /// # Errors
    ///
    /// Returns an error string describing the first check that failed.
    pub fn process_manifest(&self, manifest_json: &str) -> Result<OtaPlan, String> {
        let _guard = self.ops_lock.lock().map_err(|e| e.to_string())?;

        // 1. Authenticity first. Everything below trusts fields from this
        //    document, so nothing below may run before we know who wrote it.
        //
        //    A build with no baked key cannot answer "who wrote this", so it
        //    has no OTA — accepting an unsigned manifest instead would mean
        //    anyone who can answer the request can replace the app's code.
        //    Key generation is automatic (`manifest::ensure_keys` runs on
        //    every build), so a missing key is a misconfiguration, never a
        //    situation that needs a permissive path.
        let key = self
            .manifest_key
            .ok_or("OTA is disabled: no manifest public key was baked into this build")?;

        let manifest = crate::manifest::verify_manifest(manifest_json, &key)
            .map_err(|e| format!("manifest rejected: {e}"))?;

        // verify_manifest already enforces the schema; this is belt-and-braces
        // in case that ever stops being true.
        if manifest.schema != crate::manifest::MANIFEST_SCHEMA {
            return Err(format!("unsupported manifest schema: {}", manifest.schema));
        }
        if manifest.source != ManifestSource::Ota {
            return Err(format!(
                "expected an OTA manifest, got source={}",
                manifest.source
            ));
        }

        // 2. Routing guard (C2). The domain does not authenticate content —
        //    the signature does — but it stops a manifest from pointing the
        //    downloader at link-local metadata or an exfiltration endpoint.
        let domain = self
            .manifest_domain
            .as_deref()
            .ok_or("OTA is disabled: no manifest domain was baked into this build")?;

        if manifest.manifest_domain != domain {
            return Err(format!(
                "manifest domain mismatch: expected {domain:?}, got {:?}",
                manifest.manifest_domain
            ));
        }

        let allowed_prefix = format!("https://{domain}/");
        if !manifest.base_url.starts_with(&allowed_prefix) {
            return Err(format!(
                "base_url {:?} is not under {allowed_prefix:?}",
                manifest.base_url
            ));
        }

        // 3. Replay (H8). Equal is not greater — a manifest we already
        //    accepted must not be accepted twice.
        let last = self.last_manifest_sequence();
        if manifest.sequence <= last {
            return Err(format!(
                "manifest sequence {} is not newer than the last accepted ({last})",
                manifest.sequence
            ));
        }

        // 4. Size and count limits (H5, M7), then per-entry path validation.
        let total_files: usize = manifest.apps.iter().map(|a| a.files.len()).sum();
        if total_files > MAX_OTA_FILES {
            return Err(format!(
                "manifest declares {total_files} files, limit is {MAX_OTA_FILES}"
            ));
        }

        let base = manifest.base_url.trim_end_matches('/').to_string();
        let mut downloads = Vec::with_capacity(total_files);
        for app in &manifest.apps {
            validate_app_id(&app.app_id)?;
            validate_version(&app.bundle_version)?;

            for file in &app.files {
                validate_file_path(&file.path)?;
                if file.size > MAX_OTA_FILE_SIZE {
                    return Err(format!(
                        "{} declares {} bytes, limit is {MAX_OTA_FILE_SIZE}",
                        file.path, file.size
                    ));
                }
                if file.sha256.len() != 64 || !file.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(format!("{} has a malformed sha256", file.path));
                }

                // The URL is derived, never taken from the manifest, so a
                // per-file redirect is not expressible (C2).
                downloads.push(OtaDownload {
                    url: format!(
                        "{base}/{}/{}/{}",
                        app.app_id, app.bundle_version, file.path
                    ),
                    vfs_path: format!(
                        "{}/{}",
                        self.version_vfs_path(&app.app_id, &app.bundle_version),
                        file.path
                    ),
                    app_id: app.app_id.clone(),
                    bundle_version: app.bundle_version.clone(),
                    file: file.clone(),
                });
            }
        }

        if let Some(target) = &manifest.rollback_to {
            validate_version(target)?;
        }
        if let Some(target) = &manifest.delete_after {
            validate_version(target)?;
        }

        info!(
            sequence = manifest.sequence,
            apps = manifest.apps.len(),
            downloads = downloads.len(),
            "manifest accepted"
        );

        Ok(OtaPlan {
            manifest,
            downloads,
        })
    }

    /// Record a manifest sequence as accepted, on disk as well as in memory.
    ///
    /// Called once every download in the plan has succeeded — a manifest that
    /// failed halfway must remain replayable, or a transient network error
    /// would permanently block that update.
    ///
    /// # Errors
    ///
    /// Returns an error string if the sequence could not be persisted.
    pub fn commit_manifest_sequence(&self, sequence: u64) -> Result<(), String> {
        *self.last_manifest_seq.write().map_err(|e| e.to_string())? = sequence;
        self.fs
            .write_file(&format!("/{SEQUENCE_FILE}"), sequence.to_string().as_bytes())
            .map_err(|e| format!("persisting manifest sequence: {e}"))?;
        Ok(())
    }

    /// Write a verified manifest into a version directory as provenance.
    ///
    /// Staged through `.tmp` and renamed so a crash mid-write cannot leave a
    /// version directory claiming to be described by a truncated manifest.
    ///
    /// # Errors
    ///
    /// Returns an error string if serialization, staging, or the rename fails.
    pub fn write_version_manifest(
        &self,
        app_id: &str,
        version: &str,
        manifest: &Manifest,
    ) -> Result<(), String> {
        validate_app_id(app_id)?;
        validate_version(version)?;

        let dir = self.version_vfs_path(app_id, version);
        let final_path = format!("{dir}/{MANIFEST_FILENAME}");
        let staged = format!("{final_path}.tmp");
        let json = manifest
            .to_json()
            .map_err(|e| format!("serializing manifest: {e}"))?;

        self.write(&staged, json.as_bytes())
            .map_err(|e| format!("staging {staged}: {e}"))?;
        self.fs
            .rename(&staged, &final_path)
            .map_err(|e| format!("renaming {staged} → {final_path}: {e}"))?;
        Ok(())
    }

    /// Read the manifest describing a version directory, if one is there.
    #[must_use]
    pub fn read_version_manifest(&self, app_id: &str, version: &str) -> Option<Manifest> {
        let path = format!(
            "{}/{MANIFEST_FILENAME}",
            self.version_vfs_path(app_id, version)
        );
        let bytes = self.read(&path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
}

/// Read the persisted manifest sequence, defaulting to 0.
fn read_sequence(fs: &DynFs) -> u64 {
    fs.read_file(&format!("/{SEQUENCE_FILE}"))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

// ── Platform backend selection ──────────────────────────────────────────

/// Build the VFS backend, its mount point, and the layout for this platform.
#[cfg(target_os = "android")]
fn build_platform_vfs<R: Runtime>(app: &App<R>) -> (DynFs, PathBuf, AssetLayout) {
    use foundation_nativeapis::native::vfs::dir_delta::DirectoryDelta;
    use foundation_nativeapis::shared::vfs::overlay_fs::OverlayFileSystem;

    let app_data = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));

    // Base: the APK, read lazily. Delta: AppData, where OTA writes land.
    let base = AssetResolverFs::from_resolver(app.asset_resolver());
    match DirectoryDelta::new(&app_data) {
        Ok(delta) => {
            let overlay = OverlayFileSystem::new(base, delta);
            (wrap_audit(overlay), app_data, AssetLayout::Versioned)
        }
        Err(e) => {
            // AppData is unusable — serve the APK read-only rather than
            // refusing to start. Reads still work; OTA writes will fail
            // loudly at the point they are attempted.
            warn!(path = %app_data.display(), error = %e, "no writable delta layer; serving APK read-only");
            (wrap_audit(base), app_data, AssetLayout::Versioned)
        }
    }
}

/// Build the VFS backend, its mount point, and the layout for this platform.
#[cfg(not(target_os = "android"))]
fn build_platform_vfs<R: Runtime>(app: &App<R>) -> (DynFs, PathBuf, AssetLayout) {
    use foundation_nativeapis::native::vfs::native_fs::NativeFs;
    use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;

    // Desktop and iOS: resource_dir() is a real, readable path.
    let root = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    match NativeFs::new(&root) {
        Ok(fs) => (wrap_audit(fs), root, AssetLayout::Flat),
        Err(e) => {
            warn!(path = %root.display(), error = %e, "resource dir is not usable; falling back to an empty filesystem");
            (wrap_audit(MemoryFs::new()), root, AssetLayout::Flat)
        }
    }
}

/// Erase a backend into a [`DynFs`], wrapping it in `ObservableFs` when the
/// `vfs-audit` feature is on. Off by default — zero overhead in release.
fn wrap_audit<F>(fs: F) -> DynFs
where
    F: VfsFileSystem + Send + Sync + 'static,
    F::File: Send + Sync + 'static,
    F::SeekableFile: Send + Sync + 'static,
    F::Directory: Send + Sync + 'static,
{
    #[cfg(feature = "vfs-audit")]
    {
        use foundation_nativeapis::shared::vfs::observable_fs::ObservableFs;
        DynFs::new(Arc::new(ObservableFs::new(fs)))
    }
    #[cfg(not(feature = "vfs-audit"))]
    {
        DynFs::new(Arc::new(fs))
    }
}
