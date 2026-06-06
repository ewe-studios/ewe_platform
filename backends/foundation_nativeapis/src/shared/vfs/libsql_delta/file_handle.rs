//! File handle types for LibsqlDelta: SqliteFile, SeekableSqliteFile, SqliteDirectory.

use std::io::SeekFrom;
use std::sync::Arc;

use libsql::Connection;
use foundation_core::valtron::{execute, from_future, collect_one, Stream};

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{VfsDirectory, VfsFile, SeekableVfsFile};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};
use foundation_errstacks::ErrorTrace;

use super::types::SqliteDentry;
use super::types;
use super::path_resolve::{resolve_path_async, resolve_parent_async, subtree_inos_async};
use super::chunking::{read_chunk_range_async, write_all_chunks_async, truncate_file_async};
use super::types::file_type_to_str;

fn le(e: libsql::Error) -> ErrorTrace<VfsError> { ErrorTrace::new(types::libsql_err(e)) }

/// Bridge an async operation to sync via valtron.
fn exec_async<T: Send + 'static, F>(future: F) -> VfsResult<T>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
    F::Output: Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| ErrorTrace::new(VfsError::Backend { message: format!("valtron execution failed: {e}") }))?;
    let result: Option<Result<T, ErrorTrace<VfsError>>> = collect_one(stream);
    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => Err(ErrorTrace::new(VfsError::Backend { message: "no result from future".into() })),
    }
}

/// A file handle for a SQLite-backed file.
pub struct SqliteFile {
    pub db: Arc<Connection>,
    pub ino: i64,
    pub mode: OpenMode,
    pub chunk_size: usize,
    pub size: u64,
}

impl SqliteFile {
    fn dentry(&self) -> VfsResult<SqliteDentry> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        exec_async(async move {
            let mut stmt = conn.prepare("SELECT * FROM sqlite_dentry WHERE ino = ?").await
                .map_err(le)?;
            let row = stmt.query([ino]).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: format!("ino={ino}") }))?;
            SqliteDentry::from_row(&row)
        })
    }
}

impl VfsFile for SqliteFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        let file_size = self.size;
        let chunk_size = self.chunk_size;
        let buf_len = buf.len();
        exec_async(async move {
            let mut local_buf = vec![0u8; buf_len];
            let n = read_chunk_range_async(&conn, ino, file_size, chunk_size, &mut local_buf, offset).await?;
            Ok((n, local_buf))
        }).map(|(n, local_buf)| {
            buf[..n].copy_from_slice(&local_buf[..n]);
            n
        })
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        let chunk_size = self.chunk_size;
        let buf = buf.to_vec();
        let len = buf.len();
        exec_async(async move {
            let current_size = {
                let mut stmt = conn.prepare("SELECT size FROM sqlite_dentry WHERE ino = ?").await
                    .map_err(le)?;
                let row = stmt.query([ino]).await
                    .map_err(le)?
                    .next().await
                    .map_err(le)?
                    .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: format!("ino={ino}") }))?;
                row.get::<i64>(0).map_err(le)? as u64
            };

            let end = offset as usize + len;
            let new_size = std::cmp::max(current_size, end as u64);

            let mut stmt = conn
                .prepare("SELECT chunk_idx, data FROM sqlite_chunks WHERE ino = ? ORDER BY chunk_idx")
                .await.map_err(le)?;
            let mut rows = stmt.query([ino]).await
                .map_err(le)?;
            let mut file_data = Vec::with_capacity(current_size as usize);
            while let Some(row) = rows.next().await.map_err(le)? {
                file_data.extend(row.get::<Vec<u8>>(1).map_err(le)?);
            }
            if file_data.len() < current_size as usize {
                file_data.resize(current_size as usize, 0);
            }
            if file_data.len() < end {
                file_data.resize(end, 0);
            }
            file_data[offset as usize..end].copy_from_slice(&buf);

            let chunks: Vec<Vec<u8>> = file_data.chunks(chunk_size).map(|c| c.to_vec()).collect();
            conn.execute("DELETE FROM sqlite_chunks WHERE ino = ?", [ino])
                .await.map_err(le)?;

            let mut stmt = conn
                .prepare("INSERT INTO sqlite_chunks (ino, chunk_idx, data) VALUES (?, ?, ?)")
                .await.map_err(le)?;
            for (idx, chunk) in chunks.iter().enumerate() {
                stmt.execute((ino, idx as i64, chunk.as_slice()))
                    .await.map_err(le)?;
            }

            let checksum = blake3::hash(&file_data[..new_size as usize]);
            let updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            conn.execute(
                "UPDATE sqlite_dentry SET size = ?, checksum = ?, updated_at = ? WHERE ino = ?",
                (new_size as i64, checksum.as_bytes().to_vec(), updated_at, ino),
            ).await.map_err(le)?;

            Ok(len)
        })
    }

    fn sync_data(&self) -> VfsResult<()> {
        Ok(())
    }

    fn size(&self) -> VfsResult<u64> {
        Ok(self.size)
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        let chunk_size = self.chunk_size;
        exec_async(async move {
            truncate_file_async(&conn, ino, size, chunk_size).await
        })
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let dentry = self.dentry()?;
        Ok(dentry.to_metadata())
    }
}

/// A seekable file handle wrapping SqliteFile with cursor tracking.
pub struct SeekableSqliteFile {
    pub inner: SqliteFile,
    pub cursor: u64,
}

impl SeekableSqliteFile {
    pub fn new(inner: SqliteFile) -> Self {
        Self { inner, cursor: 0 }
    }
}

impl VfsFile for SeekableSqliteFile {
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

impl SeekableVfsFile for SeekableSqliteFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = self.inner.read_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let n = self.inner.write_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        self.cursor = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => {
                let size = self.inner.size()? as i64;
                (size + n).max(0) as u64
            }
            SeekFrom::Current(n) => {
                (self.cursor as i64 + n).max(0) as u64
            }
        };
        Ok(self.cursor)
    }

    fn position(&self) -> u64 {
        self.cursor
    }
}

/// A directory handle for SQLite-backed directories.
pub struct SqliteDirectory {
    pub db: Arc<Connection>,
    pub ino: i64,
    pub path: String,
}

impl VfsDirectory for SqliteDirectory {
    type File = SqliteFile;
    type SeekableFile = SeekableSqliteFile;

    fn path(&self) -> &str {
        &self.path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        exec_async(async move {
            let mut stmt = conn.prepare("SELECT * FROM sqlite_dentry WHERE ino = ?").await
                .map_err(le)?;
            let row = stmt.query([ino]).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: format!("ino={ino}") }))?;
            let dentry = SqliteDentry::from_row(&row)?;
            Ok(dentry.to_metadata())
        })
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        exec_async(async move {
            let mut stmt = conn
                .prepare("SELECT name, file_type FROM sqlite_dentry WHERE parent_ino = ? ORDER BY name")
                .await.map_err(le)?;
            let mut rows = stmt.query([ino]).await
                .map_err(le)?;

            let mut entries = Vec::new();
            while let Some(row) = rows.next().await.map_err(le)? {
                let name = row.get::<String>(0).map_err(le)?;
                let file_type_str = row.get::<String>(1).map_err(le)?;
                if let Some(ft) = super::types::parse_file_type(&file_type_str) {
                    entries.push(VfsDirEntry { name, file_type: ft });
                }
            }
            Ok(entries)
        })
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let conn = Arc::clone(&self.db);
        let ino = self.ino;
        let name = name.to_string();
        exec_async(async move {
            let mut stmt = conn
                .prepare("SELECT name, file_type FROM sqlite_dentry WHERE parent_ino = ? AND name = ?")
                .await.map_err(le)?;
            if let Some(row) = stmt.query((ino, name.clone())).await
                .map_err(le)?
                .next().await
                .map_err(le)?
            {
                let name = row.get::<String>(0).map_err(le)?;
                let file_type_str = row.get::<String>(1).map_err(le)?;
                if let Some(ft) = super::types::parse_file_type(&file_type_str) {
                    return Ok(Some(VfsDirEntry { name, file_type: ft }));
                }
            }
            Ok(None)
        })
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let conn = Arc::clone(&self.db);
        let parent_ino = self.ino;
        let name = name.to_string();
        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let version_id = super::types::next_version_id();
        exec_async(async move {
            conn.execute(
                "INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'file', 0, ?, 0, 0, ?, ?, ?, 65536)",
                (name.clone(), parent_ino, mode as i64, version_id.to_vec(), updated_at, updated_at),
            ).await.map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    VfsError::AlreadyExists { path: name.clone() }
                } else {
                    VfsError::Backend { message: e.to_string() }
                }
            })?;

            let mut stmt = conn
                .prepare("SELECT ino FROM sqlite_dentry WHERE parent_ino = ? AND name = ?")
                .await.map_err(le)?;
            let row = stmt.query((parent_ino, name)).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::Backend { message: "created file not found".into() }))?;
            let ino = row.get::<i64>(0).map_err(le)?;

            Ok(SqliteFile {
                db: Arc::clone(&conn),
                ino,
                mode: OpenMode::Write,
                chunk_size: 64 * 1024,
                size: 0,
            })
        })
    }

    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let conn = Arc::clone(&self.db);
        let parent_ino = self.ino;
        let name = name.to_string();
        let parent_path = self.path.clone();
        exec_async(async move {
            let updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            let version_id = super::types::next_version_id();

            let new_path = if parent_path == "/" {
                format!("/{name}")
            } else {
                format!("{parent_path}/{name}")
            };

            conn.execute(
                "INSERT INTO sqlite_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'dir', 0, 493, 0, 0, ?, ?, ?, 0)",
                (name.clone(), parent_ino, version_id.to_vec(), updated_at, updated_at),
            ).await.map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    VfsError::AlreadyExists { path: new_path.clone() }
                } else {
                    VfsError::Backend { message: e.to_string() }
                }
            })?;

            let mut stmt = conn
                .prepare("SELECT ino FROM sqlite_dentry WHERE parent_ino = ? AND name = ?")
                .await.map_err(le)?;
            let row = stmt.query((parent_ino, name)).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::Backend { message: "created dir not found".into() }))?;
            let ino = row.get::<i64>(0).map_err(le)?;

            Ok(Box::new(SqliteDirectory {
                db: Arc::clone(&conn),
                ino,
                path: new_path,
            }) as Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
        })
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let conn = Arc::clone(&self.db);
        let parent_ino = self.ino;
        let name = name.to_string();
        let path = if self.path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{}", self.path, name)
        };
        exec_async(async move {
            let mut stmt = conn
                .prepare("SELECT ino, file_type FROM sqlite_dentry WHERE parent_ino = ? AND name = ?")
                .await.map_err(le)?;
            let row = stmt.query((parent_ino, name.clone())).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;

            let ino = row.get::<i64>(0).map_err(le)?;
            let file_type = row.get::<String>(1).map_err(le)?;

            if file_type == "dir" {
                let mut count_stmt = conn
                    .prepare("SELECT COUNT(*) FROM sqlite_dentry WHERE parent_ino = ?")
                    .await.map_err(le)?;
                let count = count_stmt.query([ino]).await
                    .map_err(le)?
                    .next().await
                    .map_err(le)?
                    .map(|r| r.get::<i64>(0))
                    .transpose().map_err(le)?
                    .unwrap_or(0);
                if count > 0 {
                    return Err(ErrorTrace::new(VfsError::DirectoryNotEmpty { path }));
                }
            }

            conn.execute("DELETE FROM sqlite_dentry WHERE ino = ?", [ino])
                .await.map_err(le)?;
            Ok(())
        })
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let conn = Arc::clone(&self.db);
        let parent_ino = self.ino;
        let old_name = old_name.to_string();
        let new_name = new_name.to_string();
        exec_async(async move {
            let updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            conn.execute(
                "UPDATE sqlite_dentry SET name = ?, updated_at = ? WHERE parent_ino = ? AND name = ?",
                (new_name, updated_at, parent_ino, old_name),
            ).await.map_err(le)?;
            Ok(())
        })
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let conn = Arc::clone(&self.db);
        let path = path.to_string();
        exec_async(async move {
            let ino = resolve_path_async(conn.clone(), path.clone()).await?;
            let mut stmt = conn
                .prepare("SELECT file_type, size, chunk_size FROM sqlite_dentry WHERE ino = ?")
                .await.map_err(le)?;
            let row = stmt.query([ino]).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;

            let file_type = row.get::<String>(0).map_err(le)?;
            if file_type != "file" {
                return Err(ErrorTrace::new(VfsError::NotAFile { path }));
            }

            let size = row.get::<i64>(1).map_err(le)? as u64;
            let chunk_size = row.get::<i64>(2).map_err(le)? as usize;

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

    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let conn = Arc::clone(&self.db);
        let parent_path = self.path.clone();
        let path = path.to_string();
        exec_async(async move {
            let ino = resolve_path_async(conn.clone(), path.clone()).await?;
            let mut stmt = conn
                .prepare("SELECT file_type FROM sqlite_dentry WHERE ino = ?")
                .await.map_err(le)?;
            let row = stmt.query([ino]).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;

            let file_type = row.get::<String>(0).map_err(le)?;
            if file_type != "dir" {
                return Err(ErrorTrace::new(VfsError::NotADirectory { path }));
            }

            let full_path = if path.starts_with('/') {
                path.clone()
            } else if parent_path == "/" {
                format!("/{path}")
            } else {
                format!("{parent_path}/{path}")
            };

            Ok(Box::new(SqliteDirectory {
                db: Arc::clone(&conn),
                ino,
                path: full_path,
            }) as Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>)
        })
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let conn = Arc::clone(&self.db);
        let path = path.to_string();
        exec_async(async move {
            let ino = resolve_path_async(conn.clone(), path.clone()).await?;
            let mut stmt = conn.prepare("SELECT * FROM sqlite_dentry WHERE ino = ?").await
                .map_err(le)?;
            let row = stmt.query([ino]).await
                .map_err(le)?
                .next().await
                .map_err(le)?
                .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;
            let dentry = SqliteDentry::from_row(&row)?;
            Ok(dentry.to_metadata())
        })
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let conn = Arc::clone(&self.db);
        let path = path.to_string();
        match exec_future(resolve_path_async(conn, path.clone())) {
            Ok(_) => Ok(true),
            Err(e) if e.downcast_ref::<VfsError>().map_or(false, |v| matches!(v, VfsError::NotFound { .. })) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// Bridge for exists (same pattern as mod.rs).
fn exec_future<T: Send + 'static, F>(future: F) -> VfsResult<T>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
    F::Output: Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| ErrorTrace::new(VfsError::Backend { message: format!("valtron execution failed: {e}") }))?;
    let result: Option<Result<T, ErrorTrace<VfsError>>> = collect_one(stream);
    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => Err(ErrorTrace::new(VfsError::Backend { message: "no result from future".into() })),
    }
}

unsafe impl Send for SqliteDirectory {}
unsafe impl Sync for SqliteDirectory {}
