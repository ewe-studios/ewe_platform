use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use libsql::Connection;

use crate::shared::vfs::async_traits::{AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile};
use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::types::{OpenMode, VfsDirEntry, VfsMetadata};
use foundation_errstacks::ErrorTrace;

use super::chunking::{read_chunk_range_async, truncate_file_async};
use super::path_resolve::resolve_path_async;
use super::types;
use super::types::SqliteDentry;

fn le(e: libsql::Error) -> ErrorTrace<VfsError> {
    ErrorTrace::new(types::libsql_err(e))
}

pub struct SqliteFile {
    pub db: Arc<Connection>,
    pub ino: i64,
    pub mode: OpenMode,
    pub chunk_size: usize,
    pub size: u64,
}

impl SqliteFile {
    async fn dentry(&self) -> VfsResult<SqliteDentry> {
        let stmt = self
            .db
            .prepare("SELECT * FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query([self.ino])
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::NotFound {
                    path: format!("ino={}", self.ino),
                })
            })?;
        SqliteDentry::from_row(&row)
    }
}

#[async_trait]
impl AsyncVfsFile for SqliteFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        // Read current size from DB (may have been updated by write_at_async)
        let current_size = {
            let stmt = self
                .db
                .prepare("SELECT size FROM vfs_dentry WHERE ino = ?")
                .await
                .map_err(le)?;
            let row = stmt
                .query([self.ino])
                .await
                .map_err(le)?
                .next()
                .await
                .map_err(le)?
                .ok_or_else(|| {
                    ErrorTrace::new(VfsError::NotFound {
                        path: format!("ino={}", self.ino),
                    })
                })?;
            row.get::<i64>(0).map_err(le)? as u64
        };
        let mut buf = vec![0u8; len];
        let n =
            read_chunk_range_async(&self.db, self.ino, current_size, self.chunk_size, &mut buf, offset)
                .await?;
        buf.truncate(n);
        Ok(buf)
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        let len = data.len();
        let current_size = {
            let stmt = self
                .db
                .prepare("SELECT size FROM vfs_dentry WHERE ino = ?")
                .await
                .map_err(le)?;
            let row = stmt
                .query([self.ino])
                .await
                .map_err(le)?
                .next()
                .await
                .map_err(le)?
                .ok_or_else(|| {
                    ErrorTrace::new(VfsError::NotFound {
                        path: format!("ino={}", self.ino),
                    })
                })?;
            row.get::<i64>(0).map_err(le)? as u64
        };

        let end = offset as usize + len;
        let new_size = std::cmp::max(current_size, end as u64);

        let stmt = self
            .db
            .prepare("SELECT chunk_idx, data FROM vfs_chunks WHERE ino = ? ORDER BY chunk_idx")
            .await
            .map_err(le)?;
        let mut rows = stmt.query([self.ino]).await.map_err(le)?;
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
        file_data[offset as usize..end].copy_from_slice(&data);

        let chunks: Vec<Vec<u8>> = file_data
            .chunks(self.chunk_size)
            .map(|c| c.to_vec())
            .collect();
        self.db
            .execute("DELETE FROM vfs_chunks WHERE ino = ?", [self.ino])
            .await
            .map_err(le)?;

        // Batch insert all chunks in a single dynamic multi-row INSERT.
        // Prepared-statement reuse silently drops rows in libsql.
        if !chunks.is_empty() {
            let placeholders = chunks
                .iter()
                .map(|_| "(?, ?, ?)")
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "INSERT INTO vfs_chunks (ino, chunk_idx, data) VALUES {}",
                placeholders
            );
            let mut values: Vec<libsql::Value> = Vec::with_capacity(chunks.len() * 3);
            for (idx, chunk) in chunks.iter().enumerate() {
                values.push(self.ino.into());
                values.push((idx as i64).into());
                values.push(chunk.as_slice().to_vec().into());
            }
            self.db.execute(&sql, values).await.map_err(le)?;
        }

        let checksum = blake3::hash(&file_data[..new_size as usize]);
        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64; // millis since epoch; fits i64 until year ~292M

        self.db
            .execute(
                "UPDATE vfs_dentry SET size = ?, checksum = ?, updated_at = ? WHERE ino = ?",
                (
                    new_size as i64,
                    checksum.as_bytes().to_vec(),
                    updated_at,
                    self.ino,
                ),
            )
            .await
            .map_err(le)?;

        Ok(len)
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        Ok(())
    }

    async fn size_async(&self) -> VfsResult<u64> {
        let stmt = self
            .db
            .prepare("SELECT size FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query([self.ino])
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::NotFound {
                    path: format!("ino={}", self.ino),
                })
            })?;
        Ok(row.get::<i64>(0).map_err(le)? as u64)
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        truncate_file_async(&self.db, self.ino, size, self.chunk_size).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        let dentry = self.dentry().await?;
        Ok(dentry.to_metadata())
    }
}

pub struct SeekableSqliteFile {
    pub inner: SqliteFile,
    pub cursor: Arc<AtomicU64>,
}

impl SeekableSqliteFile {
    pub fn new(inner: SqliteFile) -> Self {
        Self {
            inner,
            cursor: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[async_trait]
impl AsyncVfsFile for SeekableSqliteFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        AsyncVfsFile::read_at_async(&self.inner, len, offset).await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        AsyncVfsFile::write_at_async(&self.inner, data, offset).await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        AsyncVfsFile::sync_data_async(&self.inner).await
    }

    async fn size_async(&self) -> VfsResult<u64> {
        AsyncVfsFile::size_async(&self.inner).await
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        AsyncVfsFile::truncate_async(&self.inner, size).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        AsyncVfsFile::metadata_async(&self.inner).await
    }
}

#[async_trait]
impl AsyncSeekableVfsFile for SeekableSqliteFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let data = AsyncVfsFile::read_at_async(&self.inner, len, self.cursor.load(Ordering::Relaxed)).await?;
        self.cursor.store(self.cursor.load(Ordering::Relaxed) + data.len() as u64, Ordering::Relaxed);
        Ok(data)
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        let n = AsyncVfsFile::write_at_async(&self.inner, data, self.cursor.load(Ordering::Relaxed)).await?;
        self.cursor.store(self.cursor.load(Ordering::Relaxed) + n as u64, Ordering::Relaxed);
        Ok(n)
    }

    async fn seek_async(&mut self, pos: std::io::SeekFrom) -> VfsResult<u64> {
        let new_cursor = match pos {
            std::io::SeekFrom::Start(n) => n,
            std::io::SeekFrom::End(n) => {
                let size = AsyncVfsFile::size_async(&self.inner).await?;
                if n >= 0 {
                    size.saturating_add(n.unsigned_abs())
                } else {
                    size.saturating_sub(n.unsigned_abs())
                }
            }
            std::io::SeekFrom::Current(n) => {
                let cur = self.cursor.load(Ordering::Relaxed);
                if n >= 0 {
                    cur.saturating_add(n.unsigned_abs())
                } else {
                    cur.saturating_sub(n.unsigned_abs())
                }
            }
        };
        self.cursor.store(new_cursor, Ordering::Relaxed);
        Ok(new_cursor)
    }

    fn position_async(&self) -> u64 {
        self.cursor.load(Ordering::Relaxed)
    }
}

pub struct SqliteDirectory {
    pub db: Arc<Connection>,
    pub ino: i64,
    pub path: String,
}

#[async_trait]
impl AsyncVfsDirectory for SqliteDirectory {
    type File = SqliteFile;
    type SeekableFile = SeekableSqliteFile;

    fn path(&self) -> String {
        self.path.clone()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        let stmt = self
            .db
            .prepare("SELECT * FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query([self.ino])
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::NotFound {
                    path: format!("ino={}", self.ino),
                })
            })?;
        let dentry = SqliteDentry::from_row(&row)?;
        Ok(dentry.to_metadata())
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let stmt = self
            .db
            .prepare(
                "SELECT name, file_type FROM vfs_dentry WHERE parent_ino = ? ORDER BY name",
            )
            .await
            .map_err(le)?;
        let mut rows = stmt.query([self.ino]).await.map_err(le)?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next().await.map_err(le)? {
            let name = row.get::<String>(0).map_err(le)?;
            let file_type_str = row.get::<String>(1).map_err(le)?;
            if let Some(ft) = super::types::parse_file_type(&file_type_str) {
                entries.push(VfsDirEntry {
                    name,
                    file_type: ft,
                });
            }
        }
        Ok(entries)
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        let stmt = self
            .db
            .prepare(
                "SELECT name, file_type FROM vfs_dentry WHERE parent_ino = ? AND name = ?",
            )
            .await
            .map_err(le)?;
        if let Some(row) = stmt
            .query((self.ino, name.clone()))
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
        {
            let name = row.get::<String>(0).map_err(le)?;
            let file_type_str = row.get::<String>(1).map_err(le)?;
            if let Some(ft) = super::types::parse_file_type(&file_type_str) {
                return Ok(Some(VfsDirEntry {
                    name,
                    file_type: ft,
                }));
            }
        }
        Ok(None)
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File> {
        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64; // millis since epoch; fits i64 until year ~292M
        let version_id = super::types::next_version_id();

        self.db
            .execute(
                "INSERT INTO vfs_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'file', 0, ?, 0, 0, ?, ?, ?, 65536)",
                (
                    name.clone(),
                    self.ino,
                    mode as i64,
                    version_id.to_vec(),
                    updated_at,
                    updated_at,
                ),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    VfsError::AlreadyExists {
                        path: name.clone(),
                    }
                } else {
                    VfsError::Backend {
                        message: e.to_string(),
                    }
                }
            })?;

        let stmt = self
            .db
            .prepare("SELECT ino FROM vfs_dentry WHERE parent_ino = ? AND name = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query((self.ino, name))
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::Backend {
                    message: "created file not found".into(),
                })
            })?;
        let ino = row.get::<i64>(0).map_err(le)?;

        Ok(SqliteFile {
            db: Arc::clone(&self.db),
            ino,
            mode: OpenMode::Write,
            chunk_size: 64 * 1024,
            size: 0,
        })
    }

    async fn create_dir_async(
        &self,
        name: String,
    ) -> VfsResult<
        Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>,
    > {
        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64; // millis since epoch; fits i64 until year ~292M
        let version_id = super::types::next_version_id();

        let new_path = if self.path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.path)
        };

        self.db
            .execute(
                "INSERT INTO vfs_dentry (name, parent_ino, file_type, size, permissions, \
                 owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
                 VALUES (?, ?, 'dir', 0, 493, 0, 0, ?, ?, ?, 0)",
                (
                    name.clone(),
                    self.ino,
                    version_id.to_vec(),
                    updated_at,
                    updated_at,
                ),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint") {
                    VfsError::AlreadyExists {
                        path: new_path.clone(),
                    }
                } else {
                    VfsError::Backend {
                        message: e.to_string(),
                    }
                }
            })?;

        let stmt = self
            .db
            .prepare("SELECT ino FROM vfs_dentry WHERE parent_ino = ? AND name = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query((self.ino, name))
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| {
                ErrorTrace::new(VfsError::Backend {
                    message: "created dir not found".into(),
                })
            })?;
        let ino = row.get::<i64>(0).map_err(le)?;

        Ok(Box::new(SqliteDirectory {
            db: Arc::clone(&self.db),
            ino,
            path: new_path,
        }))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        let path = if self.path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{}", self.path, name)
        };

        let stmt = self
            .db
            .prepare("SELECT ino, file_type FROM vfs_dentry WHERE parent_ino = ? AND name = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query((self.ino, name))
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;

        let ino = row.get::<i64>(0).map_err(le)?;
        let file_type = row.get::<String>(1).map_err(le)?;

        if file_type == "dir" {
            let count_stmt = self
                .db
                .prepare("SELECT COUNT(*) FROM vfs_dentry WHERE parent_ino = ?")
                .await
                .map_err(le)?;
            let count = count_stmt
                .query([ino])
                .await
                .map_err(le)?
                .next()
                .await
                .map_err(le)?
                .map(|r| r.get::<i64>(0))
                .transpose()
                .map_err(le)?
                .unwrap_or(0);
            if count > 0 {
                return Err(ErrorTrace::new(VfsError::DirectoryNotEmpty { path }));
            }
        }

        self.db
            .execute("DELETE FROM vfs_dentry WHERE ino = ?", [ino])
            .await
            .map_err(le)?;
        Ok(())
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64; // millis since epoch; fits i64 until year ~292M
        self.db
            .execute(
                "UPDATE vfs_dentry SET name = ?, updated_at = ? WHERE parent_ino = ? AND name = ?",
                (new_name, updated_at, self.ino, old_name),
            )
            .await
            .map_err(le)?;
        Ok(())
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        let ino = resolve_path_async(self.db.clone(), path.clone()).await?;
        let stmt = self
            .db
            .prepare("SELECT file_type, size, chunk_size FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;

        let file_type = row.get::<String>(0).map_err(le)?;
        if file_type != "file" {
            return Err(ErrorTrace::new(VfsError::NotAFile { path }));
        }

        let size = row.get::<i64>(1).map_err(le)? as u64;
        let chunk_size = row.get::<i64>(2).map_err(le)? as usize;

        Ok(SqliteFile {
            db: Arc::clone(&self.db),
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

    async fn open_directory_async(
        &self,
        path: String,
    ) -> VfsResult<
        Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>,
    > {
        let ino = resolve_path_async(self.db.clone(), path.clone()).await?;
        let stmt = self
            .db
            .prepare("SELECT file_type FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path.clone() }))?;

        let file_type = row.get::<String>(0).map_err(le)?;
        if file_type != "dir" {
            return Err(ErrorTrace::new(VfsError::NotADirectory { path }));
        }

        let full_path = if path.starts_with('/') {
            path
        } else if self.path == "/" {
            format!("/{path}")
        } else {
            format!("{}/{path}", self.path)
        };

        Ok(Box::new(SqliteDirectory {
            db: Arc::clone(&self.db),
            ino,
            path: full_path,
        }))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let ino = resolve_path_async(self.db.clone(), path.clone()).await?;
        let stmt = self
            .db
            .prepare("SELECT * FROM vfs_dentry WHERE ino = ?")
            .await
            .map_err(le)?;
        let row = stmt
            .query([ino])
            .await
            .map_err(le)?
            .next()
            .await
            .map_err(le)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path }))?;
        let dentry = SqliteDentry::from_row(&row)?;
        Ok(dentry.to_metadata())
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        match resolve_path_async(self.db.clone(), path).await {
            Ok(_) => Ok(true),
            Err(e)
                if e.downcast_ref::<VfsError>()
                    .map_or(false, |v| matches!(v, VfsError::NotFound { .. })) =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}
