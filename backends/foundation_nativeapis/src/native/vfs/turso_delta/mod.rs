//! TursoDelta — Turso cloud-backed DeltaStore.
//!
//! Uses the `turso` crate for direct connection to Turso cloud databases.
//! Suitable for Cloudflare Workers, WASM, and edge deployment scenarios
//! where the database lives remotely but SQL queries work the same.
//!
//! Unlike LibsqlDelta (which uses embedded SQLite with optional remote sync),
//! this connects directly to a Turso URL — no local file needed.

mod schema;
mod types;
mod path_resolve;
mod chunking;
mod file_handle;

use std::sync::{Arc, Mutex};

use turso::{Builder, Connection};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{DeltaStore, VfsDirectory, VfsFileSystem, SeekableVfsFile};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

use file_handle::{TursoFile, SeekableTursoFile, TursoDirectory};
use types::{ChunkConfig, TursoDentry, next_version_id, pack_version, unpack_version};
use path_resolve::{resolve_path, resolve_parent, resolve_ino_to_path, subtree_inos};
use chunking::{write_all_chunks, read_chunk_range, truncate_file};
use schema::run_migrations;

/// Configuration for TursoDelta.
#[derive(Debug, Clone)]
pub struct TursoDeltaConfig {
    /// Chunk size for file content. Default: 64 KB.
    pub chunk_config: ChunkConfig,
}

impl Default for TursoDeltaConfig {
    fn default() -> Self {
        Self {
            chunk_config: ChunkConfig::default(),
        }
    }
}

/// TursoDelta — a complete VfsFileSystem and DeltaStore backed by a Turso
/// cloud database. Suitable for edge/worker deployment.
pub struct TursoDelta {
    conn: Arc<Mutex<Connection>>,
    chunk_config: ChunkConfig,
}

impl TursoDelta {
    /// Connect to a Turso cloud database.
    ///
    /// # Errors
    ///
    /// Returns `VfsError::Io` if the connection cannot be established.
    pub fn new(url: &str, auth_token: &str) -> VfsResult<Self> {
        let db = exec_future(async move {
            Builder::new_remote(url, auth_token)
                .build()
                .await
        })?;

        let conn = db.connect()
            .map_err(|e| VfsError::Io { source: e })?;

        let delta = Self {
            conn: Arc::new(Mutex::new(conn)),
            chunk_config: ChunkConfig::default(),
        };

        delta.run_migrations()?;
        Ok(delta)
    }

    /// Connect to a Turso cloud database with custom configuration.
    pub fn with_config(url: &str, auth_token: &str, config: TursoDeltaConfig) -> VfsResult<Self> {
        let db = exec_future(async move {
            Builder::new_remote(url, auth_token)
                .build()
                .await
        })?;

        let conn = db.connect()
            .map_err(|e| VfsError::Io { source: e })?;

        let delta = Self {
            conn: Arc::new(Mutex::new(conn)),
            chunk_config: config.chunk_config,
        };

        delta.run_migrations()?;
        Ok(delta)
    }

    fn run_migrations(&self) -> VfsResult<()> {
        let conn = self.conn.lock().unwrap();
        schema::run_migrations(&conn)
    }
}

/// One-shot blocking bridge for initialization via valtron.
fn exec_future<T: Send + 'static, E: Into<VfsError> + Send + 'static, F>(
    future: F,
) -> VfsResult<T>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    F::Output: Send + 'static,
{
    use foundation_core::valtron::{collect_one, execute, from_future, Stream};

    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| VfsError::Internal(format!("valtron execution failed: {e}")))?;
    let result: Result<Option<T>, VfsError> = collect_one(stream)
        .map(|r| r.map_err(Into::into))
        .transpose();
    result?.ok_or_else(|| VfsError::Internal("no result from future execution".into()))
}

impl VfsFileSystem for TursoDelta {
    type File = TursoFile;
    type SeekableFile = SeekableTursoFile;
    type Directory = TursoDirectory;

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
        let ino = resolve_path(&self.conn, path)?;
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT * FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

        let dentry = TursoDentry::from_row(&row)?;
        Ok(dentry.to_metadata())
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        resolve_path(&self.conn, path).map(|ino| ino as u64)
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        resolve_ino_to_path(&self.conn, ino as i64)
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;
        let row = stmt
            .query((ino as i64,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: format!("inode:{ino}") })?;
        let dentry = TursoDentry::from_row(&row)?;
        Ok(dentry.to_metadata())
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        match resolve_path(&self.conn, path) {
            Ok(_) => Ok(true),
            Err(VfsError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let ino = resolve_path(&self.conn, path)?;
        let conn = self.conn.lock().unwrap();

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        conn.execute(
            "UPDATE turso_dentry SET permissions = ?, updated_at = ? WHERE ino = ?",
            (mode as i64, updated_at, ino),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn symlink(&self, target: &str, link_path: &str) -> VfsResult<()> {
        let (parent_ino, name) = resolve_parent(&self.conn, link_path)?;

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let version_id = next_version_id();

        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO turso_dentry (name, parent_ino, file_type, size, permissions, \
             owner_uid, owner_gid, version_id, created_at, updated_at, symlink_target, chunk_size) \
             VALUES (?, ?, 'symlink', 0, 0o777, 0, 0, ?, ?, ?, ?, 0)",
            (name, parent_ino, version_id.to_vec(), updated_at, updated_at, target),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let ino = resolve_path(&self.conn, path)?;
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT symlink_target FROM turso_dentry WHERE ino = ? AND file_type = 'symlink'")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotASymlink { path: path.to_string() })?;

        row.get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| VfsError::NotASymlink { path: path.to_string() })
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let ino = resolve_path(&self.conn, from)?;
        let (new_parent_ino, new_name) = resolve_parent(&self.conn, to)?;

        let conn = self.conn.lock().unwrap();

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        conn.execute(
            "UPDATE turso_dentry SET name = ?, parent_ino = ?, updated_at = ? WHERE ino = ?",
            (new_name, new_parent_ino, updated_at, ino),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let ino = resolve_path(&self.conn, path)?;

        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT file_type FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

        let file_type = row
            .get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_str()
            .unwrap_or("");

        if file_type == "dir" {
            let mut count_stmt = conn
                .prepare("SELECT COUNT(*) FROM turso_dentry WHERE parent_ino = ?")
                .map_err(|e| VfsError::Io { source: e.into() })?;

            let count = count_stmt
                .query((ino,))
                .map_err(|e| VfsError::Io { source: e.into() })?
                .next()
                .map_err(|e| VfsError::Io { source: e.into() })?
                .and_then(|r| r.get_value(0).ok().and_then(|v| v.as_integer()))
                .unwrap_or(0);

            if count > 0 {
                return Err(VfsError::DirectoryNotEmpty {
                    path: path.to_string(),
                });
            }
        }

        conn.execute("DELETE FROM turso_dentry WHERE ino = ?", (ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let ino = resolve_path(&self.conn, path)?;

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT file_type, size, chunk_size FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

        let file_type = row
            .get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_str()
            .unwrap_or("");

        if file_type != "file" {
            return Err(VfsError::NotAFile { path: path.to_string() });
        }

        let size = row
            .get_value(1)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_integer()
            .unwrap_or(0) as u64;

        let chunk_size = row
            .get_value(2)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_integer()
            .unwrap_or(65536) as usize;

        Ok(TursoFile {
            db: Arc::clone(&self.conn),
            ino,
            mode,
            chunk_size,
            size,
        })
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.open(path, mode)?;
        Ok(SeekableTursoFile::new(file))
    }

    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let ino = resolve_path(&self.conn, path)?;

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT file_type FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;

        let file_type = row
            .get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_str()
            .unwrap_or("");

        if file_type != "dir" {
            return Err(VfsError::NotADirectory { path: path.to_string() });
        }

        Ok(TursoDirectory {
            db: Arc::clone(&self.conn),
            ino,
            path: path.to_string(),
        })
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let (parent_ino, name) = resolve_parent(&self.conn, path)?;

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let version_id = next_version_id();

        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO turso_dentry (name, parent_ino, file_type, size, permissions, \
             owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
             VALUES (?, ?, 'file', 0, ?, 0, 0, ?, ?, ?, ?)",
            (
                name,
                parent_ino,
                mode as i64,
                version_id.to_vec(),
                updated_at,
                updated_at,
                self.chunk_config.default_chunk_size as i64,
            ),
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint") {
                VfsError::AlreadyExists { path: path.to_string() }
            } else {
                VfsError::Io { source: e.into() }
            }
        })?;

        let ino: i64 = {
            let mut stmt = conn
                .prepare("SELECT ino FROM turso_dentry WHERE parent_ino = ? AND name = ?")
                .map_err(|e| VfsError::Io { source: e.into() })?;
            let row = stmt
                .query((parent_ino, name))
                .map_err(|e| VfsError::Io { source: e.into() })?
                .next()
                .map_err(|e| VfsError::Io { source: e.into() })?
                .ok_or_else(|| VfsError::Internal("created file not found".into()))?;
            row.get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_integer()
                .unwrap_or(0)
        };

        Ok(TursoFile {
            db: Arc::clone(&self.conn),
            ino,
            mode: OpenMode::Write,
            chunk_size: self.chunk_config.default_chunk_size,
            size: 0,
        })
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let (parent_ino, name) = resolve_parent(&self.conn, path)?;

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let version_id = next_version_id();

        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO turso_dentry (name, parent_ino, file_type, size, permissions, \
             owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
             VALUES (?, ?, 'dir', 0, 493, 0, 0, ?, ?, ?, 0)",
            (name, parent_ino, version_id.to_vec(), updated_at, updated_at),
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint") {
                VfsError::AlreadyExists { path: path.to_string() }
            } else {
                VfsError::Io { source: e.into() }
            }
        })?;

        Ok(())
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let ino = resolve_path(&self.conn, path)?;
        write_all_chunks(&self.conn, ino, data, self.chunk_config.default_chunk_size)
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let dentry = self.stat(path)?;
        let ino = resolve_path(&self.conn, path)?;

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT data FROM turso_chunks WHERE ino = ? ORDER BY chunk_idx")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut rows = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut result = Vec::with_capacity(dentry.size as usize);
        while let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
            let chunk = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_blob()
                .unwrap_or(&[]);
            result.extend_from_slice(chunk);
        }

        Ok(result)
    }
}

impl DeltaStore for TursoDelta {
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()> {
        let version_id = pack_version(version);
        let conn = self.conn.lock().unwrap();

        let prefixes = whiteout_prefixes(path);

        conn.execute(
            "INSERT OR REPLACE INTO turso_whiteouts (path, version_id) VALUES (?, ?)",
            (path, version_id.to_vec()),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut stmt = conn
            .prepare("INSERT OR REPLACE INTO turso_whiteout_prefixes (prefix, path, version_id) VALUES (?, ?, ?)")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        for prefix in &prefixes {
            stmt.execute((prefix, path, version_id.to_vec()))
                .map_err(|e| VfsError::Io { source: e.into() })?;
        }

        Ok(())
    }

    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT version_id FROM turso_whiteouts WHERE path = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        if let Some(row) = stmt
            .query((path,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
        {
            let version_bytes = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_blob()
                .unwrap_or(&[]);

            if version_bytes.len() == 16 {
                let id: [u8; 16] = version_bytes.try_into().unwrap();
                return Ok(Some(unpack_version(id)));
            }
        }

        Ok(None)
    }

    fn remove_whiteout(&self, path: &str) -> VfsResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM turso_whiteouts WHERE path = ?", (path,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        conn.execute("DELETE FROM turso_whiteout_prefixes WHERE path = ?", (path,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>> {
        let prefix = if dir == "/" {
            "/".to_string()
        } else {
            format!("{dir}/")
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT p.path, p.version_id \
                 FROM turso_whiteout_prefixes p \
                 JOIN turso_whiteouts w ON w.path = p.path \
                 WHERE p.prefix = ? ORDER BY p.path",
            )
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut rows = stmt
            .query((prefix,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
            let path = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_str()
                .unwrap_or("")
                .to_string();

            let version_bytes = row
                .get_value(1)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_blob()
                .unwrap_or(&[]);

            if version_bytes.len() == 16 {
                let id: [u8; 16] = version_bytes.try_into().unwrap();
                result.push((path, unpack_version(id)));
            }
        }

        Ok(result)
    }

    fn flush(&self) -> VfsResult<()> {
        // Turso remote — no local WAL to checkpoint
        Ok(())
    }

    fn reset(&self) -> VfsResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM turso_chunks", ())
            .map_err(|e| VfsError::Io { source: e.into() })?;

        conn.execute("DELETE FROM turso_whiteout_prefixes", ())
            .map_err(|e| VfsError::Io { source: e.into() })?;

        conn.execute("DELETE FROM turso_whiteouts", ())
            .map_err(|e| VfsError::Io { source: e.into() })?;

        conn.execute("DELETE FROM turso_dentry WHERE ino != 1", ())
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        conn.execute(
            "UPDATE turso_dentry SET version_id = X'00000000000000000000000000000000', updated_at = ? WHERE ino = 1",
            (updated_at,),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }
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
