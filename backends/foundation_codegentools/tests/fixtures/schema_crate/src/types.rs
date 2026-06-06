#[derive(ArrowSchema, JsonSchema)]
pub struct VfsMetadata {
    pub size: u64,
    pub name: String,
    pub permissions: u32,
    pub checksum: Option<Vec<u8>>,
}

#[derive(ArrowSchema)]
pub struct DirEntry {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(JsonSchema)]
pub struct ApiResponse {
    pub status: u32,
    pub message: String,
    pub data: Option<String>,
}
