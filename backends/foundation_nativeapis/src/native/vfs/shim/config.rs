use std::env;

/// Delta store backend for the preload shim.
#[derive(Debug, Clone, Default)]
pub enum DeltaBackend {
    #[default]
    Memory,
    Sqlite,
    Turso,
    Directory,
    D1,
    R2,
}

impl std::str::FromStr for DeltaBackend {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "memory" | "mem" => Ok(DeltaBackend::Memory),
            "sqlite" | "libsql" => Ok(DeltaBackend::Sqlite),
            "turso" => Ok(DeltaBackend::Turso),
            "dir" | "directory" | "native" => Ok(DeltaBackend::Directory),
            "d1" | "cloudflare-d1" => Ok(DeltaBackend::D1),
            "r2" | "cloudflare-r2" => Ok(DeltaBackend::R2),
            _ => Err(format!(
                "unknown delta backend: {s} (use: memory, sqlite, turso, dir, d1, r2)"
            )),
        }
    }
}

#[derive(Debug)]
pub struct ShimConfig {
    pub prefixes: Vec<String>,
    pub root_dir: Option<String>,
    pub delta_backend: DeltaBackend,
    pub delta_path: Option<String>,
}

impl ShimConfig {
    pub fn from_env() -> Self {
        let prefixes = env::var("FOUNDATION_VFS_PREFIX")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let root_dir = env::var("FOUNDATION_VFS_ROOT").ok();

        let delta_backend = env::var("FOUNDATION_VFS_DELTA")
            .ok()
            .and_then(|v| v.parse::<DeltaBackend>().ok())
            .unwrap_or_default();

        let delta_path = env::var("FOUNDATION_VFS_DELTA_PATH").ok();
        Self {
            prefixes,
            root_dir,
            delta_backend,
            delta_path,
        }
    }
}
