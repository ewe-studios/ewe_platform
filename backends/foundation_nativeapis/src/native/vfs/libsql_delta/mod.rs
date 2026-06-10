mod chunking;
pub mod file_handle;
mod path_resolve;
mod schema;
pub mod types;

use std::fmt;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use libsql::{Connection, Database};

use crate::shared::vfs::async_traits::{AsyncDeltaStore, AsyncVfsDirectory, AsyncVfsFileSystem};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::exec_async::exec_async;
use crate::shared::vfs::sync_bridge::{SyncFile, SyncFs};
use crate::shared::vfs::traits::{DeltaStore, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};
use foundation_errstacks::ErrorTrace;

use chunking::write_all_chunks_async;
use file_handle::{SeekableSqliteFile, SqliteDirectory, SqliteFile};
use path_resolve::{resolve_parent_async, resolve_path_async};
use types::{next_version_id, pack_version, unpack_version, ChunkConfig, SqliteDentry};

/// Synchronous seekable file handle for LibsqlDelta.
///
/// Wraps a bridged `SyncFile<SqliteFile>` for I/O and uses an `Arc<AtomicU64>`
/// for the cursor — cheap to clone, thread-safe, no `&mut self` needed.
pub struct SyncSeekableSqliteFile {
    inner: SyncFile<SqliteFile>,
    cursor: Arc<AtomicU64>,
}

impl fmt::Debug for SyncSeekableSqliteFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncSeekableSqliteFile")
            .field("cursor", &self.cursor.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl SyncSeekableSqliteFile {
    /// Creates a new sync seekable handle from a bridged file with cursor at 0.
    #[must_use]
    pub fn new(inner: SyncFile<SqliteFile>) -> Self {
        Self {
            inner,
            cursor: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl VfsFile for SyncSeekableSqliteFile {
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

impl SeekableVfsFile for SyncSeekableSqliteFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let pos = self.cursor.load(Ordering::Relaxed);
        let n = self.inner.read_at(buf, pos)?;
        self.cursor.store(pos + n as u64, Ordering::Relaxed);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let pos = self.cursor.load(Ordering::Relaxed);
        let n = self.inner.write_at(buf, pos)?;
        self.cursor.store(pos + n as u64, Ordering::Relaxed);
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let cur = self.cursor.load(Ordering::Relaxed);
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => {
                let size = self.inner.size()?;
                if n >= 0 {
                    size.saturating_add(n.unsigned_abs())
                } else {
                    size.saturating_sub(n.unsigned_abs())
                }
            }
            SeekFrom::Current(n) => {
                if n >= 0 {
                    cur.saturating_add(n.unsigned_abs())
                } else {
                    cur.saturating_sub(n.unsigned_abs())
                }
            }
        };
        self.cursor.store(new_pos, Ordering::Relaxed);
        Ok(new_pos)
    }

    fn position(&self) -> u64 {
        self.cursor.load(Ordering::Relaxed)
    }
}

/// Synchronous wrapper around [`LibsqlDelta`] with optimized seekable support.
///
/// Delegates all `VfsFileSystem` and `DeltaStore` methods to `SyncFs<LibsqlDelta>`
/// except `open_seekable`, which returns `SyncSeekableSqliteFile` instead of the
/// generic `LocalSeekableFile`. This gives native seekable performance for libsql
/// while still using the valtron bridge for all other operations.
pub struct SyncLibsqlDelta {
    inner: SyncFs<LibsqlDelta>,
}

impl fmt::Debug for SyncLibsqlDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncLibsqlDelta").finish_non_exhaustive()
    }
}

impl SyncLibsqlDelta {
    /// Creates a new sync wrapper from an async `LibsqlDelta`.
    #[must_use]
    pub fn new(inner: LibsqlDelta) -> Self {
        Self {
            inner: SyncFs::new(inner),
        }
    }

    /// Creates a new sync wrapper from a pre-existing `Arc<LibsqlDelta>`.
    #[must_use]
    pub fn from_arc(inner: Arc<LibsqlDelta>) -> Self {
        Self {
            inner: SyncFs::from_arc(inner),
        }
    }
}

/// Synchronous directory wrapper for libsql directories with optimized seekable support.
///
/// Stores a boxed async directory and bridges all methods through `exec_async`.
/// Returns `SyncSeekableSqliteFile` instead of `LocalSeekableFile`.
pub struct SyncSqliteDirectory {
    inner: Arc<Box<dyn AsyncVfsDirectory<File = SqliteFile, SeekableFile = SeekableSqliteFile>>>,
    cached_path: String,
}

impl fmt::Debug for SyncSqliteDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncSqliteDirectory")
            .field("path", &self.cached_path)
            .finish_non_exhaustive()
    }
}

impl SyncSqliteDirectory {
    fn new(inner: Box<dyn AsyncVfsDirectory<File = SqliteFile, SeekableFile = SeekableSqliteFile>>) -> Self {
        let cached_path = inner.path();
        Self { inner: Arc::new(inner), cached_path }
    }
}

impl VfsDirectory for SyncSqliteDirectory {
    type File = SyncFile<SqliteFile>;
    type SeekableFile = SyncSeekableSqliteFile;

    fn path(&self) -> &str {
        &self.cached_path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        exec_async(async move { inner.metadata_async().await })
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let inner = self.inner.clone();
        exec_async(async move { inner.list_async().await })
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let inner = self.inner.clone();
        let name = name.to_string();
        exec_async(async move { inner.get_entry_async(name).await })
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let name = name.to_string();
        let file = exec_async(async move { inner.create_file_async(name, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let inner = self.inner.clone();
        let name = name.to_string();
        let dir = exec_async(async move { inner.create_dir_async(name).await })?;
        Ok(Box::new(SyncSqliteDirectory::new(dir)))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let name = name.to_string();
        exec_async(async move { inner.remove_entry_async(name).await })
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let inner = self.inner.clone();
        let old = old_name.to_string();
        let new = new_name.to_string();
        exec_async(async move { inner.rename_entry_async(old, new).await })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let file = exec_async(async move { inner.open_async(path, mode).await })?;
        Ok(SyncFile::new(Arc::new(file)))
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SyncSeekableSqliteFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let inner = self.inner.clone();
        let path = path.to_string();
        let dir = exec_async(async move { inner.open_directory_async(path).await })?;
        Ok(Box::new(SyncSqliteDirectory::new(dir)))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.stat_async(path).await })
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let inner = self.inner.clone();
        let path = path.to_string();
        exec_async(async move { inner.exists_async(path).await })
    }
}

impl VfsFileSystem for SyncLibsqlDelta {
    type File = <SyncFs<LibsqlDelta> as VfsFileSystem>::File;
    type SeekableFile = SyncSeekableSqliteFile;
    type Directory = SyncSqliteDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        self.inner.stat(path)
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        self.inner.inode(path)
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.inner.path_by_inode(ino)
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.inner.stat_by_inode(ino)
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        self.inner.exists(path)
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        self.inner.chmod(path, mode)
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        self.inner.symlink(target, link)
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        self.inner.readlink(path)
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        self.inner.rename(from, to)
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        self.inner.remove(path)
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        self.inner.open(path, mode)
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.inner.open(path, mode)?;
        Ok(SyncSeekableSqliteFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let inner = self.inner.inner().clone();
        let path = path.to_string();
        let dir = exec_async(async move { inner.open_directory_async(path).await })?;
        Ok(SyncSqliteDirectory::new(Box::new(dir)))
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        self.inner.create(path, mode)
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        self.inner.mkdir(path)
    }
}

impl DeltaStore for SyncLibsqlDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        self.inner.add_whiteout(path, version)
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        self.inner.is_whiteout(path)
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        self.inner.remove_whiteout(path)
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        self.inner.list_whiteouts(dir)
    }

    fn flush(&self) -> VfsResult<()> {
        self.inner.flush()
    }

    fn reset(&self) -> VfsResult<()> {
        self.inner.reset()
    }
}

/// Legacy type alias — equivalent to `SyncFs<LibsqlDelta>` with generic seekable.
/// New code should use [`SyncLibsqlDelta`] for optimized seekable support.
pub type SyncLibsqlDeltaGeneric = SyncFs<LibsqlDelta>;

/// Configuration for creating a [`LibsqlDelta`] instance.
#[derive(Debug, Clone)]
pub struct LibsqlDeltaConfig {
    /// Chunk size settings for file data storage.
    pub chunk_config: ChunkConfig,
    /// Optional Turso remote replica configuration.
    pub turso_remote: Option<TursoRemoteConfig>,
}

/// Connection details for a Turso remote replica.
#[derive(Debug, Clone)]
pub struct TursoRemoteConfig {
    /// Turso database URL (e.g. `libsql://db-org.turso.io`).
    pub url: String,
    /// Authentication token for the Turso database.
    pub auth_token: String,
}

impl Default for LibsqlDeltaConfig {
    fn default() -> Self {
        Self {
            chunk_config: ChunkConfig::default(),
            turso_remote: None,
        }
    }
}

/// SQLite-backed virtual filesystem using `libsql` with delta/overlay support.
pub struct LibsqlDelta {
    db: Option<Arc<Database>>,
    conn: Arc<Connection>,
    chunk_config: ChunkConfig,
}

/// Convert a [`libsql::Error`] into an [`ErrorTrace<VfsError>`].
fn err(e: libsql::Error) -> ErrorTrace<VfsError> {
    ErrorTrace::new(types::libsql_err(e))
}

impl LibsqlDelta {
    /// Open or create a local `libsql` database at the given path.
    ///
    /// Creates parent directories if they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Io`] if parent directory creation fails, or
    /// [`VfsError::Backend`] if the path is not valid UTF-8 or the database cannot be opened.
    pub fn new(db_path: impl AsRef<Path>) -> VfsResult<Self> {
        let path = db_path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
            }
        }
        let path_str = path
            .to_str()
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::Backend {
                    message: "database path is not valid UTF-8".into(),
                })
            })?
            .to_string();
        let db = exec_async(async move {
            libsql::Builder::new_local(&path_str)
                .build()
                .await
                .map_err(err)
        })?;
        let conn = db.connect().map_err(err)?;
        let conn_arc = Arc::new(conn);
        exec_async({
            let c = Arc::clone(&conn_arc);
            async move { c.execute_batch(&schema::CREATE_ALL).await.map_err(err) }
        })?;
        exec_async({
            let c = Arc::clone(&conn_arc);
            async move {
                for pragma in schema::PRAGMAS {
                    c.execute_batch(pragma).await.map_err(err)?;
                }
                Ok::<_, ErrorTrace<VfsError>>(())
            }
        })?;
        Ok(Self {
            db: None,
            conn: conn_arc,
            chunk_config: ChunkConfig::default(),
        })
    }

    /// Open or create a database with explicit [`LibsqlDeltaConfig`].
    ///
    /// When `config.turso_remote` is `Some`, a remote replica is created instead of a local-only database.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Io`] if parent directory creation fails, or
    /// [`VfsError::Backend`] if the database cannot be opened.
    pub fn with_config(db_path: impl AsRef<Path>, config: LibsqlDeltaConfig) -> VfsResult<Self> {
        let path = db_path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
            }
        }
        let path_str = path
            .to_str()
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::Backend {
                    message: "database path is not valid UTF-8".into(),
                })
            })?
            .to_string();
        let db = if let Some(ref remote) = config.turso_remote {
            let url = remote.url.clone();
            let token = remote.auth_token.clone();
            let ps = path_str.clone();
            exec_async(async move {
                libsql::Builder::new_remote_replica(ps, url, token)
                    .build()
                    .await
                    .map_err(err)
            })?
        } else {
            let ps = path_str;
            exec_async(async move {
                libsql::Builder::new_local(&ps)
                    .build()
                    .await
                    .map_err(err)
            })?
        };
        let conn = db.connect().map_err(err)?;
        let conn_arc = Arc::new(conn);
        exec_async({
            let c = Arc::clone(&conn_arc);
            async move { c.execute_batch(&schema::CREATE_ALL).await.map_err(err) }
        })?;
        exec_async({
            let c = Arc::clone(&conn_arc);
            async move {
                for pragma in schema::PRAGMAS {
                    c.execute_batch(pragma).await.map_err(err)?;
                }
                Ok::<_, ErrorTrace<VfsError>>(())
            }
        })?;
        Ok(Self {
            db: Some(Arc::new(db)),
            conn: conn_arc,
            chunk_config: config.chunk_config,
        })
    }

    /// Create an in-memory `libsql` database (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if the database cannot be opened.
    pub fn in_memory() -> VfsResult<Self> {
        let db = exec_async(async move {
            libsql::Builder::new_local(":memory:")
                .build()
                .await
                .map_err(err)
        })?;
        let conn = db.connect().map_err(err)?;
        let conn_arc = Arc::new(conn);
        exec_async({
            let c = Arc::clone(&conn_arc);
            async move { c.execute_batch(&schema::CREATE_ALL).await.map_err(err) }
        })?;
        Ok(Self {
            db: None,
            conn: conn_arc,
            chunk_config: ChunkConfig::default(),
        })
    }

    /// Convenience constructor for a Turso remote replica with default chunk config.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if the database or remote replica cannot be opened.
    pub fn with_turso_replica(
        local_path: impl AsRef<Path>,
        turso_url: &str,
        auth_token: &str,
    ) -> VfsResult<Self> {
        let config = LibsqlDeltaConfig {
            chunk_config: ChunkConfig::default(),
            turso_remote: Some(TursoRemoteConfig {
                url: turso_url.to_string(),
                auth_token: auth_token.to_string(),
            }),
        };
        Self::with_config(local_path, config)
    }

    /// Synchronize the local replica with the Turso remote, if configured.
    ///
    /// No-op when no remote is configured.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if the sync operation fails.
    pub fn sync_remote(&self) -> VfsResult<()> {
        if let Some(ref db) = self.db {
            let db = db.clone();
            exec_async(async move { db.sync().await.map(|_| ()).map_err(err) })?;
        }
        Ok(())
    }

    /// Consume this instance and wrap it in a synchronous [`SyncLibsqlDelta`] bridge
    /// with optimized seekable support.
    #[must_use]
    pub fn into_sync(self) -> SyncLibsqlDelta {
        SyncLibsqlDelta::new(self)
    }
}

#[async_trait]
impl AsyncVfsFileSystem for LibsqlDelta {
    type File = SqliteFile;
    type SeekableFile = SeekableSqliteFile;
    type Directory = SqliteDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities {
            seekable: true,
            symlinks: true,
            permissions_enforced: false,
            event_emission: false,
            persistent: true,
        }
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let ino = resolve_path_async(self.conn.clone(), path.clone()).await?;
        let stmt = self
            .conn
            .prepare("SELECT * FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(err)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path }))?;
        let dentry = SqliteDentry::from_row(&row)?;
        Ok(dentry.to_metadata())
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        match resolve_path_async(self.conn.clone(), path).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        let ino = resolve_path_async(self.conn.clone(), path).await?;
        let updated_at = now_ms();
        self.conn
            .execute(
                "UPDATE vfs_dentry SET permissions = ?, updated_at = ? WHERE ino = ?",
                (i64::from(mode), updated_at, ino),
            )
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn symlink_async(&self, target: String, link_path: String) -> VfsResult<()> {
        let (parent_ino, name) =
            resolve_parent_async(self.conn.clone(), link_path).await?;
        let updated_at = now_ms();
        let version_id = next_version_id();
        self.conn
            .execute(
                "INSERT INTO vfs_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, symlink_target, chunk_size) \
                 VALUES (?, ?, 'symlink', 0, 0o777, 0, 0, ?, ?, ?, ?, 0)",
                (name, parent_ino, version_id.to_vec(), updated_at, updated_at, target),
            )
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        let ino = resolve_path_async(self.conn.clone(), path.clone()).await?;
        let stmt = self
            .conn
            .prepare(
                "SELECT symlink_target FROM vfs_dentry WHERE ino = ? AND file_type = 'symlink'",
            )
            .await
            .map_err(err)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotASymlink { path }))?;
        row.get::<String>(0).map_err(err)
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        let ino = resolve_path_async(self.conn.clone(), from).await?;
        let (new_parent_ino, new_name) =
            resolve_parent_async(self.conn.clone(), to).await?;
        let updated_at = now_ms();
        self.conn
            .execute(
                "UPDATE vfs_dentry SET name = ?, parent_ino = ?, updated_at = ? WHERE ino = ?",
                (new_name, new_parent_ino, updated_at, ino),
            )
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        let ino = resolve_path_async(self.conn.clone(), path.clone()).await?;
        let stmt = self
            .conn
            .prepare("SELECT file_type FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(err)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;
        let file_type = row.get::<String>(0).map_err(err)?;
        if file_type == "dir" {
            let cs = self
                .conn
                .prepare("SELECT COUNT(*) FROM vfs_dentry WHERE parent_ino = ?")
                .await
                .map_err(err)?;
            let count = cs
                .query([ino])
                .await
                .map_err(err)?
                .next()
                .await
                .map_err(err)?
                .map(|r| r.get::<i64>(0))
                .transpose()
                .map_err(err)?
                .unwrap_or(0);
            if count > 0 {
                return Err(ErrorTrace::new(VfsError::DirectoryNotEmpty { path }));
            }
        }
        self.conn
            .execute("DELETE FROM vfs_dentry WHERE ino = ?", [ino])
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        let ino = resolve_path_async(self.conn.clone(), path.clone()).await?;
        let stmt = self
            .conn
            .prepare("SELECT file_type, size, chunk_size FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(err)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;
        let ft = row.get::<String>(0).map_err(err)?;
        if ft != "file" {
            return Err(ErrorTrace::new(VfsError::NotAFile { path }));
        }
        let size = u64::try_from(row.get::<i64>(1).map_err(err)?).unwrap_or(0);
        let chunk_size = usize::try_from(row.get::<i64>(2).map_err(err)?).unwrap_or(0);
        Ok(SqliteFile {
            db: self.conn.clone(),
            ino,
            mode,
            chunk_size,
            size,
        })
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open_async(path, mode).await?;
        Ok(SeekableSqliteFile::new(file))
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        let ino = resolve_path_async(self.conn.clone(), path.clone()).await?;
        let stmt = self
            .conn
            .prepare("SELECT file_type FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(err)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;
        let ft = row.get::<String>(0).map_err(err)?;
        if ft != "dir" {
            return Err(ErrorTrace::new(VfsError::NotADirectory {
                path: path.clone(),
            }));
        }
        Ok(SqliteDirectory {
            db: self.conn.clone(),
            ino,
            path,
        })
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        let (parent_ino, name) =
            resolve_parent_async(self.conn.clone(), path.clone()).await?;
        let updated_at = now_ms();
        let version_id = next_version_id();
        let chunk_size = i64::try_from(self.chunk_config.default_chunk_size)
            .expect("chunk size overflow");
        self.conn
            .execute(
                "INSERT INTO vfs_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'file', 0, ?, 0, 0, ?, ?, ?, ?)",
                (
                    name.clone(),
                    parent_ino,
                    mode as i64,
                    version_id.to_vec(),
                    updated_at,
                    updated_at,
                    chunk_size,
                ),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    ErrorTrace::new(VfsError::AlreadyExists { path: path.clone() })
                } else {
                    ErrorTrace::new(VfsError::Backend {
                        message: e.to_string(),
                    })
                }
            })?;
        let stmt = self
            .conn
            .prepare("SELECT ino FROM vfs_dentry WHERE parent_ino = ? AND name = ?")
            .await
            .map_err(err)?;
        let row = stmt
            .query((parent_ino, name))
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::Backend {
                    message: "created file not found".into(),
                })
            })?;
        let ino = row.get::<i64>(0).map_err(err)?;
        Ok(SqliteFile {
            db: self.conn.clone(),
            ino,
            mode: OpenMode::Write,
            chunk_size: chunk_size as usize,
            size: 0,
        })
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        let (parent_ino, name) =
            resolve_parent_async(self.conn.clone(), path.clone()).await?;
        let updated_at = now_ms();
        let version_id = next_version_id();
        self.conn
            .execute(
                "INSERT INTO vfs_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'dir', 0, 493, 0, 0, ?, ?, ?, 0)",
                (
                    name,
                    parent_ino,
                    version_id.to_vec(),
                    updated_at,
                    updated_at,
                ),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    ErrorTrace::new(VfsError::AlreadyExists { path })
                } else {
                    ErrorTrace::new(VfsError::Backend {
                        message: e.to_string(),
                    })
                }
            })?;
        Ok(())
    }

    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        let ino = resolve_path_async(self.conn.clone(), path).await?;
        write_all_chunks_async(&self.conn, ino, &data, self.chunk_config.default_chunk_size)
            .await
    }

    async fn read_file_async(&self, path: String) -> VfsResult<Vec<u8>> {
        let dentry = self.stat_async(path.clone()).await?;
        let ino = resolve_path_async(self.conn.clone(), path).await?;
        let stmt = self
            .conn
            .prepare("SELECT data FROM vfs_chunks WHERE ino = ? ORDER BY chunk_idx")
            .await
            .map_err(err)?;
        let mut rows = stmt.query([ino]).await.map_err(err)?;
        let mut result = Vec::with_capacity(dentry.size as usize);
        while let Some(row) = rows.next().await.map_err(err)? {
            result.extend(row.get::<Vec<u8>>(0).map_err(err)?);
        }
        Ok(result)
    }
}

#[async_trait]
impl AsyncDeltaStore for LibsqlDelta {
    async fn add_whiteout_async(&self, path: String, version: u64) -> VfsResult<()> {
        let version_id = pack_version(version);
        let prefixes = whiteout_prefixes(&path);

        // Insert whiteout entry
        self.conn
            .execute(
                "INSERT OR REPLACE INTO vfs_whiteouts (path, version_id) VALUES (?, ?)",
                (path.clone(), version_id.to_vec()),
            )
            .await
            .map_err(err)?;

        // Batch insert all prefix entries in a single dynamic multi-row INSERT.
        // We must NOT use a prepared statement here — libsql silently drops rows
        // when a prepared Statement is reused in a loop.
        if !prefixes.is_empty() {
            let placeholders = prefixes
                .iter()
                .map(|_| "(?, ?, ?)")
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "INSERT OR REPLACE INTO vfs_whiteout_prefixes (prefix, path, version_id) VALUES {}",
                placeholders
            );
            let version_bytes = version_id.to_vec();
            let mut values: Vec<libsql::Value> = Vec::with_capacity(prefixes.len() * 3);
            for prefix in &prefixes {
                values.push(prefix.clone().into());
                values.push(path.clone().into());
                values.push(version_bytes.clone().into());
            }
            self.conn.execute(&sql, values).await.map_err(err)?;
        }

        Ok(())
    }

    async fn is_whiteout_async(&self, path: String) -> VfsResult<Option<u64>> {
        let stmt = self
            .conn
            .prepare("SELECT version_id FROM vfs_whiteouts WHERE path = ?")
            .await
            .map_err(err)?;
        if let Some(row) = stmt
            .query([path])
            .await
            .map_err(err)?
            .next()
            .await
            .map_err(err)?
        {
            let vb = row.get::<Vec<u8>>(0).map_err(err)?;
            if vb.len() == 16 {
                return Ok(Some(unpack_version(vb.try_into().unwrap())));
            }
        }
        Ok(None)
    }

    async fn remove_whiteout_async(&self, path: String) -> VfsResult<()> {
        self.conn
            .execute(
                "DELETE FROM vfs_whiteouts WHERE path = ?",
                [path.clone()],
            )
            .await
            .map_err(err)?;
        self.conn
            .execute(
                "DELETE FROM vfs_whiteout_prefixes WHERE path = ?",
                [path],
            )
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn list_whiteouts_async(&self, dir: String) -> VfsResult<Vec<(String, u64)>> {
        let prefix = if dir == "/" {
            "/".to_string()
        } else {
            dir
        };
        let stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT w.path, w.version_id FROM vfs_whiteouts w \
                 JOIN vfs_whiteout_prefixes p ON p.path = w.path WHERE p.prefix = ? ORDER BY w.path",
            )
            .await
            .map_err(err)?;
        let mut rows = stmt.query([prefix]).await.map_err(err)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(err)? {
            let path = row.get::<String>(0).map_err(err)?;
            let vb = row.get::<Vec<u8>>(1).map_err(err)?;
            if vb.len() == 16 {
                result.push((path, unpack_version(vb.try_into().unwrap())));
            }
        }
        Ok(result)
    }

    async fn flush_async(&self) -> VfsResult<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn reset_async(&self) -> VfsResult<()> {
        let updated_at = now_ms();
        self.conn
            .execute("DELETE FROM vfs_chunks", ())
            .await
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM vfs_whiteout_prefixes", ())
            .await
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM vfs_whiteouts", ())
            .await
            .map_err(err)?;
        self.conn
            .execute("DELETE FROM vfs_dentry WHERE ino != 1", ())
            .await
            .map_err(err)?;
        self.conn
            .execute(
                "UPDATE vfs_dentry SET version_id = X'00000000000000000000000000000000', \
                 updated_at = ? WHERE ino = 1",
                [updated_at],
            )
            .await
            .map_err(err)?;
        Ok(())
    }
}

/// Current time as milliseconds since the Unix epoch.
///
/// # Panics
///
/// Panics if the system clock is set before the Unix epoch.
fn now_ms() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis();
    i64::try_from(millis).expect("timestamp overflow")
}

fn whiteout_prefixes(path: &str) -> Vec<String> {
    let normalized = path.trim_start_matches('/');
    if normalized.is_empty() {
        return vec!["/".to_string()];
    }
    let mut prefixes = vec![format!("/{normalized}")];
    let parts: Vec<&str> = normalized.split('/').collect();
    for i in 1..parts.len() {
        prefixes.push(format!("/{}", parts[..i].join("/")));
        prefixes.push(format!("/{}/", parts[..i].join("/")));
    }
    prefixes.push("/".to_string());
    prefixes
}
