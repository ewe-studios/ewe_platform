//! LibsqlDelta — local SQLite DeltaStore via libsql.
//!
//! Uses `libsql::Builder::new_local()` for embedded SQLite with WAL mode.
//! Optional Turso remote sync via `libsql::Builder::new_remote_replica()`.
//!
//! libsql is async-only, so we bridge via valtron's `from_future` + `execute`
//! pattern, matching foundation_db's libsql_store approach.

mod chunking;
pub mod file_handle;
mod path_resolve;
mod schema;
pub mod types;

use std::path::Path;
use std::sync::Arc;

use libsql::{Connection, Database};
use foundation_core::valtron::{
    collect_one, execute, from_future, Stream, StreamIteratorExt, ThreadedValue,
};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsFileSystem, VfsDirectory, SeekableVfsFile};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

use file_handle::{SqliteDirectory, SqliteFile, SeekableSqliteFile};
use types::{ChunkConfig, SqliteDentry};
use path_resolve::{resolve_path_async, resolve_parent_async, subtree_inos_async};
use chunking::{write_all_chunks_async, read_chunk_range_async, truncate_file_async};
use types::{next_version_id, pack_version, unpack_version};

/// Configuration for LibsqlDelta.
#[derive(Debug, Clone)]
pub struct LibsqlDeltaConfig {
    pub chunk_config: ChunkConfig,
    pub turso_remote: Option<TursoRemoteConfig>,
}

#[derive(Debug, Clone)]
pub struct TursoRemoteConfig {
    pub url: String,
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

/// LibsqlDelta — a complete VfsFileSystem and DeltaStore backed by a local
/// SQLite database file via libsql.
pub struct LibsqlDelta {
    db: Option<Database>,
    conn: Arc<Connection>,
    chunk_config: ChunkConfig,
}

/// One-shot blocking bridge for initialization via valtron.
fn exec_future<T: Send + 'static, F>(
    future: F,
) -> VfsResult<T>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
    F::Output: Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| VfsError::Backend { message: format!("valtron execution failed: {e}") }.into())?;
    let result: Result<Option<T>, foundation_errstacks::ErrorTrace<VfsError>> = collect_one(stream);
    result?.ok_or_else(|| VfsError::Backend { message: "no result from future execution".into() }.into())
}

/// Schedule an async operation, returning an iterator of results.
fn schedule_future<T: Send + 'static, F>(
    future: F,
) -> VfsResult<Box<dyn Iterator<Item = ThreadedValue<Result<T, VfsError>>> + Send>>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
    F::Output: Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| VfsError::Backend { message: format!("valtron scheduling failed: {e}") })?;

    Ok(Box::new(
        stream.map(|item| match item {
            Stream::Next(result) => ThreadedValue::Value(result.map_err(Into::into)),
            Stream::Pending(()) => ThreadedValue::Waiting,
            _ => ThreadedValue::Value(Err(VfsError::Backend { message: "stream ended unexpectedly".into() })),
        }),
    ))
}

/// Helper: wrap an async result into a VfsResult iterator via valtron.
fn wrap_async<T: Send + 'static>(
    future: impl std::future::Future<Output = VfsResult<T>> + Send + 'static,
) -> VfsResult<Box<dyn Iterator<Item = ThreadedValue<Result<T, VfsError>>> + Send>> {
    schedule_future(future)
}

fn to_threaded_iter<T: Send + 'static>(
    stream: impl foundation_core::valtron::StreamIterator<D = Result<T, VfsError>, P = ()> + Send + 'static,
) -> Box<dyn Iterator<Item = ThreadedValue<Result<T, VfsError>>> + Send> {
    Box::new(stream.filter_map(|item| match item {
        Stream::Next(result) => Some(ThreadedValue::Value(result)),
        _ => None,
    }))
}

impl LibsqlDelta {
    pub fn new(db_path: impl AsRef<Path>) -> VfsResult<Self> {
        let path = db_path.as_ref();

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(types::libsql_err)?;
            }
        }

        let path_str = path.to_str().ok_or_else(|| VfsError::Backend {
            message: "database path is not valid UTF-8".into(),
        })?;

        let db = exec_future(async move {
            libsql::Builder::new_local(path_str).build().await
        })?;

        let conn = db.connect()
            .map_err(types::libsql_err)?;

        let delta = Self {
            db: None,
            conn: Arc::new(conn),
            chunk_config: ChunkConfig::default(),
        };

        exec_future(async {
            delta.conn.clone().execute_batch(&schema::CREATE_ALL).await
                .map_err(types::libsql_err)
        })?;

        exec_future(async {
            let conn = delta.conn.clone();
            for pragma in schema::PRAGMAS {
                conn.execute(pragma, ()).await
                    .map_err(types::libsql_err)?;
            }
            Ok::<_, VfsError>(())
        })?;

        Ok(delta)
    }

    pub fn with_config(db_path: impl AsRef<Path>, config: LibsqlDeltaConfig) -> VfsResult<Self> {
        let path = db_path.as_ref();

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(types::libsql_err)?;
            }
        }

        let path_str = path.to_str().ok_or_else(|| VfsError::Backend {
            message: "database path is not valid UTF-8".into(),
        })?;

        let db = if let Some(ref remote) = config.turso_remote {
            exec_future(async move {
                libsql::Builder::new_remote_replica(path_str, &remote.url, &remote.auth_token)
                    .build().await
            })?
        } else {
            exec_future(async move {
                libsql::Builder::new_local(path_str).build().await
            })?
        };

        let conn = db.connect()
            .map_err(types::libsql_err)?;

        let delta = Self {
            db: Some(db),
            conn: Arc::new(conn),
            chunk_config: config.chunk_config,
        };

        exec_future(async {
            delta.conn.clone().execute_batch(&schema::CREATE_ALL).await
                .map_err(types::libsql_err)
        })?;

        exec_future(async {
            let conn = delta.conn.clone();
            for pragma in schema::PRAGMAS {
                conn.execute(pragma, ()).await
                    .map_err(types::libsql_err)?;
            }
            Ok::<_, VfsError>(())
        })?;

        Ok(delta)
    }

    pub fn in_memory() -> VfsResult<Self> {
        let db = exec_future(async move {
            libsql::Builder::new_local(":memory:").build().await
        })?;

        let conn = db.connect()
            .map_err(types::libsql_err)?;

        let delta = Self {
            db: None,
            conn: Arc::new(conn),
            chunk_config: ChunkConfig::default(),
        };

        exec_future(async {
            delta.conn.clone().execute_batch(&schema::CREATE_ALL).await
                .map_err(types::libsql_err)
        })?;

        Ok(delta)
    }

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

    pub fn sync_remote(&self) -> VfsResult<()> {
        if let Some(ref db) = self.db {
            let db = Arc::clone(db);
            exec_future(async move {
                db.sync().await.map(|_| ()).map_err(types::libsql_err)
            })?;
        }
        Ok(())
    }
}

impl VfsFileSystem for LibsqlDelta {
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

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn.prepare("SELECT * FROM sqlite_dentry WHERE ino = ?").await
                .map_err(types::libsql_err)?;
            let row = stmt.query([ino]).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
                .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;
            let dentry = SqliteDentry::from_row(&row)?;
            Ok(dentry.to_metadata())
        })
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        match exec_future(resolve_path_async(Arc::clone(&self.conn), path)) {
            Ok(_) => Ok(true),
            Err(VfsError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        let updated_at = now_ms();
        exec_future(async move {
            conn.execute(
                "UPDATE sqlite_dentry SET permissions = ?, updated_at = ? WHERE ino = ?",
                (mode as i64, updated_at, ino),
            ).await.map_err(types::libsql_err)?;
            Ok(())
        })
    }

    fn symlink(&self, target: &str, link_path: &str) -> VfsResult<()> {
        let (parent_ino, name) = exec_future(resolve_parent_async(Arc::clone(&self.conn), link_path))?;
        let conn = Arc::clone(&self.conn);
        let updated_at = now_ms();
        let version_id = next_version_id();
        exec_future(async move {
            conn.execute(
                "INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, symlink_target, chunk_size) \
                 VALUES (?, ?, 'symlink', 0, 0o777, 0, 0, ?, ?, ?, ?, 0)",
                (name, parent_ino, version_id.to_vec(), updated_at, updated_at, target),
            ).await.map_err(types::libsql_err)?;
            Ok(())
        })
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn
                .prepare("SELECT symlink_target FROM sqlite_dentry WHERE ino = ? AND file_type = 'symlink'").await
                .map_err(types::libsql_err)?;
            let row = stmt.query([ino]).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
                .ok_or_else(|| VfsError::NotASymlink { path: path.to_string() })?;
            row.get::<String>(0)
                .map_err(types::libsql_err)
        })
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), from))?;
        let (new_parent_ino, new_name) = exec_future(resolve_parent_async(Arc::clone(&self.conn), to))?;
        let conn = Arc::clone(&self.conn);
        let updated_at = now_ms();
        exec_future(async move {
            conn.execute(
                "UPDATE sqlite_dentry SET name = ?, parent_ino = ?, updated_at = ? WHERE ino = ?",
                (new_name, new_parent_ino, updated_at, ino),
            ).await.map_err(types::libsql_err)?;
            Ok(())
        })
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn.prepare("SELECT file_type FROM sqlite_dentry WHERE ino = ?").await
                .map_err(types::libsql_err)?;
            let row = stmt.query([ino]).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
                .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

            let file_type = row.get::<String>(0)
                .map_err(types::libsql_err)?;

            if file_type == "dir" {
                let mut count_stmt = conn
                    .prepare("SELECT COUNT(*) FROM sqlite_dentry WHERE parent_ino = ?").await
                    .map_err(types::libsql_err)?;
                let count = count_stmt.query([ino]).await
                    .map_err(types::libsql_err)?
                    .next().await
                    .map_err(types::libsql_err)?
                    .map(|r| r.get::<i64>(0))
                    .transpose().map_err(types::libsql_err)?
                    .unwrap_or(0);

                if count > 0 {
                    return Err(VfsError::DirectoryNotEmpty { path: path.to_string() });
                }
            }

            conn.execute("DELETE FROM sqlite_dentry WHERE ino = ?", [ino]).await
                .map_err(types::libsql_err)?;
            Ok(())
        })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn
                .prepare("SELECT file_type, size, chunk_size FROM sqlite_dentry WHERE ino = ?").await
                .map_err(types::libsql_err)?;
            let row = stmt.query([ino]).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
                .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

            let file_type = row.get::<String>(0)
                .map_err(types::libsql_err)?;
            if file_type != "file" {
                return Err(VfsError::NotAFile { path: path.to_string() });
            }

            let size = row.get::<i64>(1)
                .map_err(types::libsql_err)? as u64;
            let chunk_size = row.get::<i64>(2)
                .map_err(types::libsql_err)? as usize;

            Ok(SqliteFile {
                db: Arc::clone(&conn),
                ino,
                mode,
                chunk_size,
                size,
            })
        })
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableSqliteFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn.prepare("SELECT file_type FROM sqlite_dentry WHERE ino = ?").await
                .map_err(types::libsql_err)?;
            let row = stmt.query([ino]).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
                .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

            let file_type = row.get::<String>(0)
                .map_err(types::libsql_err)?;
            if file_type != "dir" {
                return Err(VfsError::NotADirectory { path: path.to_string() });
            }

            Ok(SqliteDirectory {
                db: Arc::clone(&conn),
                ino,
                path: path.to_string(),
            })
        })
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let (parent_ino, name) = exec_future(resolve_parent_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        let updated_at = now_ms();
        let version_id = next_version_id();
        let chunk_size = self.chunk_config.default_chunk_size as i64;
        let path_str = path.to_string();
        exec_future(async move {
            conn.execute(
                "INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'file', 0, ?, 0, 0, ?, ?, ?, ?)",
                (name.clone(), parent_ino, mode as i64, version_id.to_vec(), updated_at, updated_at, chunk_size),
            ).await.map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    VfsError::AlreadyExists { path: path_str.clone() }
                } else {
                    VfsError::Backend { message: e.to_string() }
                }
            })?;

            let mut stmt = conn
                .prepare("SELECT ino FROM sqlite_dentry WHERE parent_ino = ? AND name = ?").await
                .map_err(types::libsql_err)?;
            let row = stmt.query((parent_ino, name)).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
                .ok_or_else(|| VfsError::Backend { message: "created file not found".into() })?;
            let ino = row.get::<i64>(0)
                .map_err(types::libsql_err)?;

            Ok(SqliteFile {
                db: Arc::clone(&conn),
                ino,
                mode: OpenMode::Write,
                chunk_size: chunk_size as usize,
                size: 0,
            })
        })
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let (parent_ino, name) = exec_future(resolve_parent_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        let updated_at = now_ms();
        let version_id = next_version_id();
        let path_str = path.to_string();
        exec_future(async move {
            conn.execute(
                "INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'dir', 0, 493, 0, 0, ?, ?, ?, 0)",
                (name, parent_ino, version_id.to_vec(), updated_at, updated_at),
            ).await.map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    VfsError::AlreadyExists { path: path_str }
                } else {
                    VfsError::Backend { message: e.to_string() }
                }
            })?;
            Ok(())
        })
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let chunk_size = self.chunk_config.default_chunk_size;
        let conn = Arc::clone(&self.conn);
        let data = data.to_vec();
        exec_future(async move {
            write_all_chunks_async(&conn, ino, &data, chunk_size).await
        })
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let dentry = self.stat(path)?;
        let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path))?;
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn
                .prepare("SELECT data FROM sqlite_chunks WHERE ino = ? ORDER BY chunk_idx").await
                .map_err(types::libsql_err)?;
            let mut rows = stmt.query([ino]).await
                .map_err(types::libsql_err)?;

            let mut result = Vec::with_capacity(dentry.size as usize);
            while let Some(row) = rows.next().await
                .map_err(types::libsql_err)?
            {
                let chunk = row.get::<Vec<u8>>(0)
                    .map_err(types::libsql_err)?;
                result.extend_from_slice(&chunk);
            }
            Ok(result)
        })
    }
}

impl DeltaStore for LibsqlDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let version_id = pack_version(version);
        let prefixes = whiteout_prefixes(path);
        let conn = Arc::clone(&self.conn);
        let path_str = path.to_string();
        exec_future(async move {
            conn.execute(
                "INSERT OR REPLACE INTO sqlite_whiteouts (path, version_id) VALUES (?, ?)",
                (path_str.clone(), version_id.to_vec()),
            ).await.map_err(types::libsql_err)?;

            let mut stmt = conn
                .prepare("INSERT OR REPLACE INTO sqlite_whiteout_prefixes (prefix, path, version_id) VALUES (?, ?, ?)").await
                .map_err(types::libsql_err)?;

            for prefix in &prefixes {
                stmt.execute((prefix, path_str.clone(), version_id.to_vec())).await
                    .map_err(types::libsql_err)?;
            }
            Ok(())
        })
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        let conn = Arc::clone(&self.conn);
        let path_str = path.to_string();
        exec_future(async move {
            let mut stmt = conn
                .prepare("SELECT version_id FROM sqlite_whiteouts WHERE path = ?").await
                .map_err(types::libsql_err)?;
            if let Some(row) = stmt.query([path_str]).await
                .map_err(types::libsql_err)?
                .next().await
                .map_err(types::libsql_err)?
            {
                let version_bytes = row.get::<Vec<u8>>(0)
                    .map_err(types::libsql_err)?;
                if version_bytes.len() == 16 {
                    let id: [u8; 16] = version_bytes.try_into().unwrap();
                    return Ok(Some(unpack_version(id)));
                }
            }
            Ok(None)
        })
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        let conn = Arc::clone(&self.conn);
        let path_str = path.to_string();
        exec_future(async move {
            conn.execute("DELETE FROM sqlite_whiteouts WHERE path = ?", [path_str.clone()]).await
                .map_err(types::libsql_err)?;
            conn.execute("DELETE FROM sqlite_whiteout_prefixes WHERE path = ?", [path_str]).await
                .map_err(types::libsql_err)?;
            Ok(())
        })
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        let prefix = if dir == "/" {
            "/".to_string()
        } else {
            format!("{dir}/")
        };
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT p.path, p.version_id \
                     FROM sqlite_whiteout_prefixes p \
                     JOIN sqlite_whiteouts w ON w.path = p.path \
                     WHERE p.prefix = ? ORDER BY p.path",
                ).await.map_err(types::libsql_err)?;

            let mut rows = stmt.query([prefix]).await
                .map_err(types::libsql_err)?;

            let mut result = Vec::new();
            while let Some(row) = rows.next().await
                .map_err(types::libsql_err)?
            {
                let path = row.get::<String>(0)
                    .map_err(types::libsql_err)?;
                let version_bytes = row.get::<Vec<u8>>(1)
                    .map_err(types::libsql_err)?;
                if version_bytes.len() == 16 {
                    let id: [u8; 16] = version_bytes.try_into().unwrap();
                    result.push((path, unpack_version(id)));
                }
            }
            Ok(result)
        })
    }

    fn flush(&self) -> VfsResult<()> {
        let conn = Arc::clone(&self.conn);
        exec_future(async move {
            conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", ()).await
                .map_err(types::libsql_err)?;
            Ok(())
        })
    }

    fn reset(&self) -> VfsResult<()> {
        let conn = Arc::clone(&self.conn);
        let updated_at = now_ms();
        exec_future(async move {
            conn.execute("DELETE FROM sqlite_chunks", ()).await
                .map_err(types::libsql_err)?;
            conn.execute("DELETE FROM sqlite_whiteout_prefixes", ()).await
                .map_err(types::libsql_err)?;
            conn.execute("DELETE FROM sqlite_whiteouts", ()).await
                .map_err(types::libsql_err)?;
            conn.execute("DELETE FROM sqlite_dentry WHERE ino != 1", ()).await
                .map_err(types::libsql_err)?;
            conn.execute(
                "UPDATE sqlite_dentry SET version_id = X'00000000000000000000000000000000', updated_at = ? WHERE ino = 1",
                [updated_at],
            ).await.map_err(types::libsql_err)?;
            Ok(())
        })
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Generate hierarchical prefix entries for a whiteout path.
fn whiteout_prefixes(path: &str) -> Vec<String> {
    let normalized = path.trim_start_matches('/');
    if normalized.is_empty() {
        return vec!["/".to_string()];
    }

    let mut prefixes = vec![format!("/{normalized}")];
    let parts: Vec<&str> = normalized.split('/').collect();
    for i in 1..parts.len() {
        let prefix = format!("/{}", parts[..i].join("/"));
        prefixes.push(format!("{prefix}/"));
    }
    prefixes.push("/".to_string());
    prefixes
}
