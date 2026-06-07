use std::io::SeekFrom;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::types::{
    Checksum, OpenMode, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType, VfsMetadata,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VfsRequest {
    // File operations
    Open { path: String, mode: IpcOpenMode, seekable: bool },
    Create { path: String, mode: u32 },
    ReadAt { handle: u64, offset: u64, size: u32 },
    WriteAt { handle: u64, #[serde(with = "serde_bytes")] data: Vec<u8>, offset: u64 },
    Seek { handle: u64, pos: IpcSeekFrom },
    Truncate { handle: u64, size: u64 },
    SyncData { handle: u64 },
    CloseFile { handle: u64 },
    FileSize { handle: u64 },
    FileMetadata { handle: u64 },

    // Path operations
    Stat { path: String },
    Exists { path: String },
    Mkdir { path: String },
    Remove { path: String },
    Rename { from: String, to: String },
    Chmod { path: String, mode: u32 },
    Symlink { target: String, link: String },
    ReadLink { path: String },

    // Inode operations
    Inode { path: String },
    PathByInode { ino: u64 },
    StatByInode { ino: u64 },

    // Directory handle operations
    OpenDir { path: String },
    DirList { handle: u64 },
    DirGetEntry { handle: u64, name: String },
    DirPath { handle: u64 },
    DirMetadata { handle: u64 },
    DirCreateFile { handle: u64, name: String, mode: u32 },
    DirCreateDir { handle: u64, name: String },
    DirRemoveEntry { handle: u64, name: String },
    CloseDir { handle: u64 },

    // Convenience
    ReadFile { path: String },
    WriteFile { path: String, #[serde(with = "serde_bytes")] data: Vec<u8> },

    // Capabilities
    Capabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VfsResponse {
    FileHandle(u64),
    DirHandle(u64),
    Data(#[serde(with = "serde_bytes")] Vec<u8>),
    Metadata(IpcVfsMetadata),
    DirEntries(Vec<IpcVfsDirEntry>),
    DirEntry(Option<IpcVfsDirEntry>),
    Exists(bool),
    Position(u64),
    Size(u64),
    BytesTransferred(usize),
    Capabilities(IpcVfsCapabilities),
    Inode(u64),
    Path(String),
    Ok,
    Error(String),
}

// ── Serializable mirror types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IpcOpenMode {
    Read,
    Write,
    ReadWrite,
}

impl From<OpenMode> for IpcOpenMode {
    fn from(m: OpenMode) -> Self {
        match m {
            OpenMode::Read => Self::Read,
            OpenMode::Write => Self::Write,
            OpenMode::ReadWrite => Self::ReadWrite,
        }
    }
}

impl From<IpcOpenMode> for OpenMode {
    fn from(m: IpcOpenMode) -> Self {
        match m {
            IpcOpenMode::Read => Self::Read,
            IpcOpenMode::Write => Self::Write,
            IpcOpenMode::ReadWrite => Self::ReadWrite,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IpcSeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

impl From<SeekFrom> for IpcSeekFrom {
    fn from(s: SeekFrom) -> Self {
        match s {
            SeekFrom::Start(n) => Self::Start(n),
            SeekFrom::End(n) => Self::End(n),
            SeekFrom::Current(n) => Self::Current(n),
        }
    }
}

impl From<IpcSeekFrom> for SeekFrom {
    fn from(s: IpcSeekFrom) -> Self {
        match s {
            IpcSeekFrom::Start(n) => Self::Start(n),
            IpcSeekFrom::End(n) => Self::End(n),
            IpcSeekFrom::Current(n) => Self::Current(n),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IpcVfsFileType {
    Regular,
    Directory,
    Symlink,
}

impl From<VfsFileType> for IpcVfsFileType {
    fn from(t: VfsFileType) -> Self {
        match t {
            VfsFileType::Regular => Self::Regular,
            VfsFileType::Directory => Self::Directory,
            VfsFileType::Symlink => Self::Symlink,
        }
    }
}

impl From<IpcVfsFileType> for VfsFileType {
    fn from(t: IpcVfsFileType) -> Self {
        match t {
            IpcVfsFileType::Regular => Self::Regular,
            IpcVfsFileType::Directory => Self::Directory,
            IpcVfsFileType::Symlink => Self::Symlink,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IpcVfsEntryState {
    Ready,
    Pending,
}

impl From<VfsEntryState> for IpcVfsEntryState {
    fn from(s: VfsEntryState) -> Self {
        match s {
            VfsEntryState::Ready => Self::Ready,
            VfsEntryState::Pending => Self::Pending,
        }
    }
}

impl From<IpcVfsEntryState> for VfsEntryState {
    fn from(s: IpcVfsEntryState) -> Self {
        match s {
            IpcVfsEntryState::Ready => Self::Ready,
            IpcVfsEntryState::Pending => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcChecksum {
    Blake3([u8; 32]),
    None,
}

impl From<Checksum> for IpcChecksum {
    fn from(c: Checksum) -> Self {
        match c {
            Checksum::Blake3(h) => Self::Blake3(h),
            Checksum::None => Self::None,
        }
    }
}

impl From<IpcChecksum> for Checksum {
    fn from(c: IpcChecksum) -> Self {
        match c {
            IpcChecksum::Blake3(h) => Self::Blake3(h),
            IpcChecksum::None => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcVfsMetadata {
    pub inode: u64,
    pub size: u64,
    pub file_type: IpcVfsFileType,
    pub permissions: u32,
    pub owner: (u32, u32),
    pub created_ms: Option<u64>,
    pub modified_ms: Option<u64>,
    pub accessed_ms: Option<u64>,
    pub checksum: IpcChecksum,
    pub version: u64,
    pub state: IpcVfsEntryState,
}

fn systemtime_to_ms(t: Option<SystemTime>) -> Option<u64> {
    t.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

fn ms_to_systemtime(ms: Option<u64>) -> Option<SystemTime> {
    ms.map(|ms| SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms))
}

impl From<VfsMetadata> for IpcVfsMetadata {
    fn from(m: VfsMetadata) -> Self {
        Self {
            inode: m.inode,
            size: m.size,
            file_type: m.file_type.into(),
            permissions: m.permissions,
            owner: m.owner,
            created_ms: systemtime_to_ms(m.created),
            modified_ms: systemtime_to_ms(m.modified),
            accessed_ms: systemtime_to_ms(m.accessed),
            checksum: m.checksum.into(),
            version: m.version,
            state: m.state.into(),
        }
    }
}

impl From<IpcVfsMetadata> for VfsMetadata {
    fn from(m: IpcVfsMetadata) -> Self {
        Self {
            inode: m.inode,
            size: m.size,
            file_type: m.file_type.into(),
            permissions: m.permissions,
            owner: m.owner,
            created: ms_to_systemtime(m.created_ms),
            modified: ms_to_systemtime(m.modified_ms),
            accessed: ms_to_systemtime(m.accessed_ms),
            checksum: m.checksum.into(),
            version: m.version,
            state: m.state.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcVfsDirEntry {
    pub inode: u64,
    pub name: String,
    pub file_type: IpcVfsFileType,
}

impl From<VfsDirEntry> for IpcVfsDirEntry {
    fn from(e: VfsDirEntry) -> Self {
        Self {
            inode: e.inode,
            name: e.name,
            file_type: e.file_type.into(),
        }
    }
}

impl From<IpcVfsDirEntry> for VfsDirEntry {
    fn from(e: IpcVfsDirEntry) -> Self {
        Self {
            inode: e.inode,
            name: e.name,
            file_type: e.file_type.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IpcVfsCapabilities {
    pub seekable: bool,
    pub symlinks: bool,
    pub permissions_enforced: bool,
    pub event_emission: bool,
    pub persistent: bool,
}

impl From<VfsCapabilities> for IpcVfsCapabilities {
    fn from(c: VfsCapabilities) -> Self {
        Self {
            seekable: c.seekable,
            symlinks: c.symlinks,
            permissions_enforced: c.permissions_enforced,
            event_emission: c.event_emission,
            persistent: c.persistent,
        }
    }
}

impl From<IpcVfsCapabilities> for VfsCapabilities {
    fn from(c: IpcVfsCapabilities) -> Self {
        Self {
            seekable: c.seekable,
            symlinks: c.symlinks,
            permissions_enforced: c.permissions_enforced,
            event_emission: c.event_emission,
            persistent: c.persistent,
        }
    }
}
