use crate::shared::vfs::types::VfsFileType;

#[derive(Debug, Clone)]
pub struct R2ObjectMeta {
    pub key: String,
    pub size: u64,
    pub etag: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct R2FsConfig {
    pub bucket: String,
    pub prefix: String,
    pub layout: KeyLayout,
}

#[derive(Debug, Clone, Default)]
pub enum KeyLayout {
    #[default]
    Path,
    ContentAddressed,
    DirectoryManifest,
}

impl R2FsConfig {
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            prefix: String::new(),
            layout: KeyLayout::Path,
        }
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub fn with_layout(mut self, layout: KeyLayout) -> Self {
        self.layout = layout;
        self
    }
}

pub trait KeyLayoutAdapter: Send + Sync + std::fmt::Debug {
    fn vfs_path_to_key(&self, path: &str) -> String;
    fn key_to_vfs_path(&self, key: &str) -> String;
    fn list_prefix(&self, dir_path: &str) -> String;
}

#[derive(Debug)]
pub struct PathKeyLayout {
    prefix: String,
}

impl PathKeyLayout {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }
}

impl KeyLayoutAdapter for PathKeyLayout {
    fn vfs_path_to_key(&self, path: &str) -> String {
        let stripped = path.trim_start_matches('/');
        if self.prefix.is_empty() {
            stripped.to_string()
        } else {
            format!("{}/{}", self.prefix, stripped)
        }
    }

    fn key_to_vfs_path(&self, key: &str) -> String {
        let stripped = if self.prefix.is_empty() {
            key
        } else {
            key.strip_prefix(&self.prefix)
                .and_then(|s| s.strip_prefix('/'))
                .unwrap_or(key)
        };
        format!("/{}", stripped)
    }

    fn list_prefix(&self, dir_path: &str) -> String {
        let stripped = dir_path.trim_start_matches('/');
        if self.prefix.is_empty() {
            if stripped.is_empty() {
                String::new()
            } else {
                format!("{}/", stripped)
            }
        } else if stripped.is_empty() {
            format!("{}/", self.prefix)
        } else {
            format!("{}/{}/", self.prefix, stripped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_layout_roundtrip() {
        let layout = PathKeyLayout::new("");
        assert_eq!(layout.vfs_path_to_key("/src/main.rs"), "src/main.rs");
        assert_eq!(layout.key_to_vfs_path("src/main.rs"), "/src/main.rs");
    }

    #[test]
    fn path_layout_with_prefix() {
        let layout = PathKeyLayout::new("overlay-v1");
        assert_eq!(layout.vfs_path_to_key("/src/main.rs"), "overlay-v1/src/main.rs");
        assert_eq!(layout.key_to_vfs_path("overlay-v1/src/main.rs"), "/src/main.rs");
    }

    #[test]
    fn list_prefix_root() {
        let layout = PathKeyLayout::new("");
        assert_eq!(layout.list_prefix("/"), "");
    }

    #[test]
    fn list_prefix_subdir() {
        let layout = PathKeyLayout::new("delta");
        assert_eq!(layout.list_prefix("/src"), "delta/src/");
    }
}
