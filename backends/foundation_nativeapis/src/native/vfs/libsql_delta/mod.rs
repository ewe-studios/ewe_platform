mod chunking;
pub mod file_handle;
mod path_resolve;
mod schema;
pub mod types;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use libsql::{Connection, Database};

use crate::shared::vfs::async_traits::{AsyncDeltaStore, AsyncVfsFileSystem};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsMetadata};
use foundation_errstacks::ErrorTrace;

use chunking::write_all_chunks_async;
use file_handle::{SeekableSqliteFile, SqliteDirectory, SqliteFile};
use path_resolve::{resolve_parent_async, resolve_path_async};
use types::{next_version_id, pack_version, unpack_version, ChunkConfig, SqliteDentry};

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
    /// The database; kept open for the life of the store.
    db: Arc<Database>,
    /// `true` for a Turso remote replica, which `sync_remote` synchronizes.
    replica: bool,
    conn: Arc<Connection>,
    chunk_config: ChunkConfig,
}

/// Convert a [`libsql::Error`] into an [`ErrorTrace<VfsError>`].
fn err(e: libsql::Error) -> ErrorTrace<VfsError> {
    ErrorTrace::new(types::libsql_err(e))
}

/// Creates the parent directories of `path` and returns it as UTF-8.
fn prepare_db_path(path: &Path) -> VfsResult<String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ErrorTrace::new(VfsError::Io { source: e }))?;
        }
    }
    path.to_str().map(str::to_string).ok_or_else(|| {
        ErrorTrace::new(VfsError::Backend {
            message: "database path is not valid UTF-8".into(),
        })
    })
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
    pub async fn new(db_path: impl AsRef<Path>) -> VfsResult<Self> {
        Self::with_config(
            db_path,
            LibsqlDeltaConfig {
                chunk_config: ChunkConfig::default(),
                turso_remote: None,
            },
        )
        .await
    }

    /// Open or create a database with explicit [`LibsqlDeltaConfig`].
    ///
    /// When `config.turso_remote` is `Some`, a remote replica is created instead of a local-only database.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Io`] if parent directory creation fails, or
    /// [`VfsError::Backend`] if the database cannot be opened.
    pub async fn with_config(
        db_path: impl AsRef<Path>,
        config: LibsqlDeltaConfig,
    ) -> VfsResult<Self> {
        let path_str = prepare_db_path(db_path.as_ref())?;
        let db = if let Some(ref remote) = config.turso_remote {
            libsql::Builder::new_remote_replica(
                path_str,
                remote.url.clone(),
                remote.auth_token.clone(),
            )
            .build()
            .await
            .map_err(err)?
        } else {
            libsql::Builder::new_local(&path_str)
                .build()
                .await
                .map_err(err)?
        };
        let replica = config.turso_remote.is_some();
        Self::open_db(db, config.chunk_config, true, replica).await
    }

    /// Create an in-memory `libsql` database (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if the database cannot be opened.
    pub async fn in_memory() -> VfsResult<Self> {
        let db = libsql::Builder::new_local(":memory:")
            .build()
            .await
            .map_err(err)?;
        Self::open_db(db, ChunkConfig::default(), false, false).await
    }

    /// Connects, creates the schema and, for a file database, sets the pragmas.
    async fn open_db(
        db: Database,
        chunk_config: ChunkConfig,
        pragmas: bool,
        replica: bool,
    ) -> VfsResult<Self> {
        let conn = db.connect().map_err(err)?;
        conn.execute_batch(&schema::CREATE_ALL).await.map_err(err)?;
        if pragmas {
            for pragma in schema::PRAGMAS {
                conn.execute_batch(pragma).await.map_err(err)?;
            }
        }
        Ok(Self {
            db: Arc::new(db),
            replica,
            conn: Arc::new(conn),
            chunk_config,
        })
    }

    /// Convenience constructor for a Turso remote replica with default chunk config.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if the database or remote replica cannot be opened.
    pub async fn with_turso_replica(
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
        Self::with_config(local_path, config).await
    }

    /// Synchronize the local replica with the Turso remote, if configured.
    ///
    /// No-op for a database without a remote.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if the sync operation fails.
    pub async fn sync_remote(&self) -> VfsResult<()> {
        if self.replica {
            self.db.sync().await.map_err(err)?;
        }
        Ok(())
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

    /// Replaces the file's content, and creates the file (mode `0o644`) when
    /// it does not exist, as the trait's default does.
    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        if !self.exists_async(path.clone()).await? {
            drop(self.create_async(path.clone(), 0o644).await?);
        }
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
