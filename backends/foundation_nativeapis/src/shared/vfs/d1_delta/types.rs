use crate::shared::vfs::types::VfsFileType;

#[derive(Debug, Clone)]
pub struct D1FileMeta {
    pub ino: u64,
    pub name: String,
    pub parent_ino: u64,
    pub file_type: VfsFileType,
    pub size: u64,
    pub permissions: u32,
    pub owner: (u32, u32),
    pub checksum: Option<String>,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct D1ChunkRef {
    pub ino: u64,
    pub chunk_idx: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct D1FsConfig {
    pub chunk_size: usize,
}

impl Default for D1FsConfig {
    fn default() -> Self {
        Self { chunk_size: 4096 }
    }
}
