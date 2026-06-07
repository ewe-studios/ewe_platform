//! File handle types for TursoDelta: TursoFile, SeekableTursoFile, TursoDirectory.

use std::io::SeekFrom;
use std::sync::{Arc, Mutex};

use turso::Connection;

use crate::shared::vfs::error::{VfsError, VfsResult};
use crate::shared::vfs::traits::{VfsDirectory, VfsFile, SeekableVfsFile};
use crate::shared::vfs::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

use super::types::TursoDentry;
use super::path_resolve::{resolve_path, resolve_parent, subtree_inos};
use super::chunking::{read_chunk_range, write_chunk_data, truncate_file};
use super::types::file_type_to_str;

/// A file handle for a SQLite-backed file.
pub struct TursoFile {
    pub db: Arc<Mutex<Connection>>,
    pub ino: i64,
    pub mode: OpenMode,
    pub chunk_size: usize,
    pub size: u64,
}

impl TursoFile {
    fn dentry(&self) -> VfsResult<TursoDentry> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((self.ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: format!("ino={}", self.ino) })?;

        TursoDentry::from_row(&row)
    }
}

impl VfsFile for TursoFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        read_chunk_range(&self.db, self.ino, self.size, self.chunk_size, buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let new_size = write_chunk_data(&self.db, self.ino, self.chunk_size, buf, offset)?;
        // Update local size
        unsafe {
            let size_ptr = &self.size as *const u64 as *mut u64;
            *size_ptr = new_size;
        }
        Ok(buf.len())
    }

    fn sync_data(&self) -> VfsResult<()> {
        // SQLite writes are immediately durable on commit; no-op
        Ok(())
    }

    fn size(&self) -> VfsResult<u64> {
        Ok(self.size)
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        truncate_file(&self.db, self.ino, size, self.chunk_size)?;
        unsafe {
            let size_ptr = &self.size as *const u64 as *mut u64;
            *size_ptr = size;
        }
        Ok(())
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let dentry = self.dentry()?;
        Ok(dentry.to_metadata())
    }
}

/// A seekable file handle wrapping TursoFile with cursor tracking.
pub struct SeekableTursoFile {
    pub inner: TursoFile,
    pub cursor: u64,
}

impl SeekableTursoFile {
    pub fn new(inner: TursoFile) -> Self {
        Self { inner, cursor: 0 }
    }
}

impl VfsFile for SeekableTursoFile {
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

impl SeekableVfsFile for SeekableTursoFile {
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
pub struct TursoDirectory {
    pub db: Arc<Mutex<Connection>>,
    pub ino: i64,
    pub path: String,
}

impl VfsDirectory for TursoDirectory {
    type File = TursoFile;
    type SeekableFile = SeekableTursoFile;

    fn path(&self) -> &str {
        &self.path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((self.ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: self.path.clone() })?;

        let dentry = TursoDentry::from_row(&row)?;
        Ok(dentry.to_metadata())
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name, file_type, ino FROM turso_dentry WHERE parent_ino = ? ORDER BY name")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut rows = stmt
            .query((self.ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut entries = Vec::new();
        while let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
            let name = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_str()
                .unwrap_or("")
                .to_string();
            let file_type_str = row
                .get_value(1)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_str()
                .unwrap_or("file");
            let ino = row
                .get_value(2)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_integer()
                .unwrap_or(0) as u64;

            if let Some(ft) = super::types::parse_file_type(file_type_str) {
                entries.push(VfsDirEntry { inode: ino, name, file_type: ft });
            }
        }

        Ok(entries)
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name, file_type, ino FROM turso_dentry WHERE parent_ino = ? AND name = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        if let Some(row) = stmt
            .query((self.ino, name))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
        {
            let name = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_str()
                .unwrap_or("")
                .to_string();
            let file_type_str = row
                .get_value(1)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_str()
                .unwrap_or("file");
            let ino = row
                .get_value(2)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_integer()
                .unwrap_or(0) as u64;

            if let Some(ft) = super::types::parse_file_type(file_type_str) {
                return Ok(Some(VfsDirEntry { inode: ino, name, file_type: ft }));
            }
        }

        Ok(None)
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File> {
        let conn = self.db.lock().unwrap();

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let version_id = super::types::next_version_id();

        conn.execute(
            "INSERT INTO turso_dentry (name, parent_ino, file_type, size, permissions, \
             owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
             VALUES (?, ?, 'file', 0, ?, 0, 0, ?, ?, ?, 65536)",
            (name, self.ino, mode as i64, version_id.to_vec(), updated_at, updated_at),
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint") {
                VfsError::AlreadyExists { path: format!("{}/{}", self.path, name) }
            } else {
                VfsError::Io { source: e.into() }
            }
        })?;

        // Get the ino of the newly created file
        let ino: i64 = {
            let mut stmt = conn
                .prepare("SELECT ino FROM turso_dentry WHERE parent_ino = ? AND name = ?")
                .map_err(|e| VfsError::Io { source: e.into() })?;
            let row = stmt
                .query((self.ino, name))
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
            db: Arc::clone(&self.db),
            ino,
            mode: OpenMode::Write,
            chunk_size: 64 * 1024,
            size: 0,
        })
    }

    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let conn = self.db.lock().unwrap();

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let version_id = super::types::next_version_id();

        let new_path = if self.path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{}", self.path, name)
        };

        conn.execute(
            "INSERT INTO turso_dentry (name, parent_ino, file_type, size, permissions, \
             owner_uid, owner_gid, version_id, created_at, updated_at, chunk_size) \
             VALUES (?, ?, 'dir', 0, 493, 0, 0, ?, ?, ?, 65536)",
            (name, self.ino, version_id.to_vec(), updated_at, updated_at),
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint") {
                VfsError::AlreadyExists { path: new_path.clone() }
            } else {
                VfsError::Io { source: e.into() }
            }
        })?;

        let ino: i64 = {
            let mut stmt = conn
                .prepare("SELECT ino FROM turso_dentry WHERE parent_ino = ? AND name = ?")
                .map_err(|e| VfsError::Io { source: e.into() })?;
            let row = stmt
                .query((self.ino, name))
                .map_err(|e| VfsError::Io { source: e.into() })?
                .next()
                .map_err(|e| VfsError::Io { source: e.into() })?
                .ok_or_else(|| VfsError::Internal("created dir not found".into()))?;
            row.get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_integer()
                .unwrap_or(0)
        };

        Ok(Box::new(TursoDirectory {
            db: Arc::clone(&self.db),
            ino,
            path: new_path,
        }))
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        let conn = self.db.lock().unwrap();

        // Get ino of the entry
        let mut stmt = conn
            .prepare("SELECT ino, file_type FROM turso_dentry WHERE parent_ino = ? AND name = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((self.ino, name))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: format!("{}/{}", self.path, name) })?;

        let ino = row
            .get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_integer()
            .unwrap_or(0);

        let file_type = row
            .get_value(1)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_str()
            .unwrap_or("file");

        // If directory, check it's empty
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
                    path: format!("{}/{}", self.path, name),
                });
            }
        }

        conn.execute("DELETE FROM turso_dentry WHERE ino = ?", (ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let conn = self.db.lock().unwrap();

        let updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        conn.execute(
            "UPDATE turso_dentry SET name = ?, updated_at = ? WHERE parent_ino = ? AND name = ?",
            (new_name, updated_at, self.ino, old_name),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        Ok(())
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let ino = resolve_path(&self.db, path)?;

        let conn = self.db.lock().unwrap();
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
            db: Arc::clone(&self.db),
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

    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>> {
        let ino = resolve_path(&self.db, path)?;

        let conn = self.db.lock().unwrap();
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

        let full_path = if self.path == "/" {
            if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{}", path)
            }
        } else {
            if path.starts_with('/') {
                path.to_string()
            } else {
                format!("{}/{}", self.path, path)
            }
        };

        Ok(Box::new(TursoDirectory {
            db: Arc::clone(&self.db),
            ino,
            path: full_path,
        }))
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let ino = resolve_path(&self.db, path)?;

        let conn = self.db.lock().unwrap();
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

    fn exists(&self, path: &str) -> VfsResult<bool> {
        match resolve_path(&self.db, path) {
            Ok(_) => Ok(true),
            Err(VfsError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

unsafe impl Send for TursoDirectory {}
unsafe impl Sync for TursoDirectory {}
