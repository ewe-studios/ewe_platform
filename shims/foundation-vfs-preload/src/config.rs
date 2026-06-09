use std::env;

#[derive(Debug)]
pub struct ShimConfig {
    pub prefixes: Vec<String>,
    pub socket_path: Option<String>,
}

impl ShimConfig {
    pub fn from_env() -> Self {
        let prefixes = env::var("FOUNDATION_VFS_PREFIX")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let socket_path = env::var("FOUNDATION_VFS_SOCKET").ok();

        Self {
            prefixes,
            socket_path,
        }
    }
}
