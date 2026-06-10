use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VfsFileType {
    Regular,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(into = "ChecksumSerde", from = "ChecksumSerde")]
pub enum Checksum {
    Blake3([u8; 32]),
    None,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum ChecksumSerde {
    Blake3(String),
    None,
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{:02x}", b)).collect()
}

fn from_hex(s: &str) -> [u8; 32] {
    let bytes: Vec<u8> = (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i+2], 16).ok())
        .collect();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes[..bytes.len().min(32)]);
    arr
}

impl From<Checksum> for ChecksumSerde {
    fn from(c: Checksum) -> Self {
        match c {
            Checksum::Blake3(b) => ChecksumSerde::Blake3(to_hex(&b)),
            Checksum::None => ChecksumSerde::None,
        }
    }
}

impl From<ChecksumSerde> for Checksum {
    fn from(s: ChecksumSerde) -> Self {
        match s {
            ChecksumSerde::Blake3(h) => Checksum::Blake3(from_hex(&h)),
            ChecksumSerde::None => Checksum::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VfsEntryState {
    Ready,
    Pending,
}

/// Serializes SystemTime as seconds since UNIX epoch.
mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::SystemTime;

    pub fn serialize<S: Serializer>(v: &Option<SystemTime>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(t) => t.duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .serialize(s),
            None => Option::<u64>::None.serialize(s),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<SystemTime>, D::Error> {
        let opt = Option::<u64>::deserialize(d)?;
        Ok(opt.map(|secs| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VfsMetadata {
    pub inode: u64,
    pub size: u64,
    pub file_type: VfsFileType,
    pub permissions: u32,
    pub owner: (u32, u32),
    #[serde(with = "system_time_serde", default)]
    pub created: Option<SystemTime>,
    #[serde(with = "system_time_serde", default)]
    pub modified: Option<SystemTime>,
    #[serde(with = "system_time_serde", default)]
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
