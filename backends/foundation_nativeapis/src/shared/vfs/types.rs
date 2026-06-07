use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsFileType {
    Regular,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    Blake3([u8; 32]),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsEntryState {
    Ready,
    Pending,
}

#[derive(Debug, Clone)]
pub struct VfsMetadata {
    pub inode: u64,
    pub size: u64,
    pub file_type: VfsFileType,
    pub permissions: u32,
    pub owner: (u32, u32),
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub checksum: Checksum,
    pub version: u64,
    pub state: VfsEntryState,
}

impl VfsMetadata {
    pub fn new_file(inode: u64, size: u64, permissions: u32) -> Self {
        let now = Some(SystemTime::now());
        Self {
            inode,
            size,
            file_type: VfsFileType::Regular,
            permissions,
            owner: (0, 0),
            created: now,
            modified: now,
            accessed: now,
            checksum: Checksum::None,
            version: 0,
            state: VfsEntryState::Ready,
        }
    }

    pub fn new_directory(inode: u64, permissions: u32) -> Self {
        let now = Some(SystemTime::now());
        Self {
            inode,
            size: 0,
            file_type: VfsFileType::Directory,
            permissions,
            owner: (0, 0),
            created: now,
            modified: now,
            accessed: now,
            checksum: Checksum::None,
            version: 0,
            state: VfsEntryState::Ready,
        }
    }

    pub fn new_symlink(inode: u64) -> Self {
        let now = Some(SystemTime::now());
        Self {
            inode,
            size: 0,
            file_type: VfsFileType::Symlink,
            permissions: 0o777,
            owner: (0, 0),
            created: now,
            modified: now,
            accessed: now,
            checksum: Checksum::None,
            version: 0,
            state: VfsEntryState::Ready,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VfsDirEntry {
    pub inode: u64,
    pub name: String,
    pub file_type: VfsFileType,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VfsCapabilities {
    pub seekable: bool,
    pub symlinks: bool,
    pub permissions_enforced: bool,
    pub event_emission: bool,
    pub persistent: bool,
}

pub use std::io::SeekFrom;
