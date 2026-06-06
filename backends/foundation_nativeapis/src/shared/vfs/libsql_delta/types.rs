//! Types for LibsqlDelta: SqliteDentry, ChunkConfig, and mapping helpers.

use std::time::{Duration, SystemTime};

use libsql::Row;

use crate::shared::vfs::types::{Checksum, VfsEntryState, VfsFileType, VfsMetadata};
use crate::shared::vfs::error::{VfsError, VfsResult};
use foundation_errstacks::ErrorTrace;

/// Convert a libsql error to a VfsError.
pub fn libsql_err(e: libsql::Error) -> VfsError {
    VfsError::Backend { message: e.to_string() }
}

/// Chunk configuration.
#[derive(Debug, Clone, Copy)]
pub struct ChunkConfig {
    /// Default chunk size in bytes. Default: 64 KB.
    pub default_chunk_size: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            default_chunk_size: 64 * 1024,
        }
    }
}

/// Rust-side representation of a vfs_dentry row.
#[derive(Debug, Clone)]
pub struct SqliteDentry {
    pub ino: i64,
    pub name: String,
    pub parent_ino: i64,
    pub file_type: VfsFileType,
    pub size: u64,
    pub permissions: u32,
    pub owner_uid: u32,
    pub owner_gid: u32,
    pub checksum: Option<[u8; 32]>,
    pub version_id: [u8; 16],
    pub created_at: u64,
    pub updated_at: u64,
    pub symlink_target: Option<String>,
    pub chunk_size: u32,
}

impl SqliteDentry {
    /// Extract a SqliteDentry from a libsql Row.
    pub fn from_row(row: &Row) -> VfsResult<Self> {
        let ino = row.get::<i64>(0).map_err(libsql_err)?;
        let name = row.get::<String>(1).map_err(libsql_err)?;
        let parent_ino = row.get::<i64>(2).map_err(libsql_err)?;
        let file_type_str = row.get::<String>(3).map_err(libsql_err)?;

        let file_type = match file_type_str.as_str() {
            "file" => VfsFileType::Regular,
            "dir" => VfsFileType::Directory,
            "symlink" => VfsFileType::Symlink,
            other => return Err(ErrorTrace::new(VfsError::Backend { message: format!("unknown file_type: {other}") })),
        };

        let size = row.get::<i64>(4).map_err(libsql_err)? as u64;
        let permissions = row.get::<i64>(5).map_err(libsql_err)? as u32;
        let owner_uid = row.get::<i64>(6).map_err(libsql_err)? as u32;
        let owner_gid = row.get::<i64>(7).map_err(libsql_err)? as u32;

        let checksum = match row.get::<Vec<u8>>(8) {
            Ok(bytes) => {
                let arr: [u8; 32] = bytes.try_into().map_err(|_| {
                    VfsError::Backend { message: "checksum is not 32 bytes".into() }
                })?;
                Some(arr)
            }
            Err(_) => None,
        };

        let version_id = match row.get::<Vec<u8>>(9) {
            Ok(bytes) => {
                bytes.try_into().map_err(|_| {
                    VfsError::Backend { message: "version_id is not 16 bytes".into() }
                })?
            }
            Err(_) => [0u8; 16],
        };

        let created_at = row.get::<i64>(10).map_err(libsql_err)? as u64;
        let updated_at = row.get::<i64>(11).map_err(libsql_err)? as u64;

        let symlink_target = row.get::<String>(12).ok();

        let chunk_size = row.get::<i64>(13).map_err(libsql_err)? as u32;

        Ok(Self {
            ino,
            name,
            parent_ino,
            file_type,
            size,
            permissions,
            owner_uid,
            owner_gid,
            checksum,
            version_id,
            created_at,
            updated_at,
            symlink_target,
            chunk_size,
        })
    }

    /// Convert to VfsMetadata.
    pub fn to_metadata(&self) -> VfsMetadata {
        let version = unpack_version(self.version_id);
        VfsMetadata {
            size: self.size,
            file_type: self.file_type,
            permissions: self.permissions,
            owner: (self.owner_uid, self.owner_gid),
            created: Some(SystemTime::UNIX_EPOCH + Duration::from_millis(self.created_at)),
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_millis(self.updated_at)),
            accessed: None,
            checksum: self.checksum.map(Checksum::Blake3).unwrap_or(Checksum::None),
            version,
            state: VfsEntryState::Ready,
        }
    }
}

/// Generate a new SCRU128 version ID.
pub fn next_version_id() -> [u8; 16] {
    scru128::new().to_bytes()
}

/// Pack an overlay u64 version into a SCRU128 ID.
pub fn pack_version(overlay_version: u64) -> [u8; 16] {
    let ts = scru128::new().timestamp();
    let counter = (overlay_version & 0xFF_FFFF) as u32;
    // try_from_fields expects: timestamp(48-bit), counter_hi(24-bit), counter_lo(24-bit), entropy(32-bit)
    scru128::Id::try_from_fields(ts, counter, 0, 0)
        .expect("valid scru128 fields")
        .to_bytes()
}

/// Extract u64 from SCRU128 ID, preserving ordering.
pub fn unpack_version(id: [u8; 16]) -> u64 {
    let scru = scru128::Id::from_bytes(id);
    let ts = scru.timestamp();
    let counter_hi = scru.counter_hi();
    (ts << 24) | counter_hi as u64
}

/// Parse file_type string to VfsFileType.
pub fn parse_file_type(s: &str) -> Option<VfsFileType> {
    match s {
        "file" => Some(VfsFileType::Regular),
        "dir" => Some(VfsFileType::Directory),
        "symlink" => Some(VfsFileType::Symlink),
        _ => None,
    }
}

/// Convert VfsFileType to string for SQL storage.
pub fn file_type_to_str(ft: VfsFileType) -> &'static str {
    match ft {
        VfsFileType::Regular => "file",
        VfsFileType::Directory => "dir",
        VfsFileType::Symlink => "symlink",
    }
}
