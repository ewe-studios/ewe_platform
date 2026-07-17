---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F10-binary-downloaders"
this_file: "specifications/58-foundation-daemonmaster/features/F10-binary-downloaders/feature.md"

status: planned
priority: high
created: 2026-07-18

depends_on: []

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# F10 — Binary downloaders (GitHub releases, crates.io, arbitrary URLs)

## Overview

Provider-based download system for binaries and artifacts. Download from GitHub
releases, crates.io, or arbitrary HTTP endpoints into managed directories. The
downloaded binary path feeds directly into a `DaemonDef::run` config so the daemon
supervisor can keep it running.

**New crate: `foundation_downloaders`** — kept separate from `foundation_nativeapis`
since downloading is orthogonal to process supervision and useful to other crates.

[spec](../spec.md).

---

## Part A — Provider trait

```rust
// foundation_downloaders/src/lib.rs

use std::path::PathBuf;

/// A download source that can resolve artifacts to a local path.
#[async_trait]
pub trait Downloader: Send + Sync {
    /// Download the artifact to `dest_dir`.
    /// Returns the path to the downloaded file(s).
    async fn download(&self, dest_dir: &PathBuf) -> Result<DownloadResult, DownloadError>;
}

/// Result of a download operation.
pub struct DownloadResult {
    /// Path to the primary artifact (binary, archive, etc.).
    pub artifact_path: PathBuf,
    /// Additional files unpacked alongside (config, licenses, etc.).
    pub extra_files: Vec<PathBuf>,
    /// Version or identifier of the downloaded artifact.
    pub version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("version not found: {0}")]
    VersionNotFound(String),
    #[error("extraction error: {0}")]
    Extract(#[from] std::io::Error),
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}
```

---

## Part B — GitHub Releases downloader

```rust
// foundation_downloaders/src/github.rs

pub struct GitHubRelease {
    /// Owner/repo (e.g. "cloudflare/cloudflared").
    pub repo: String,
    /// Tag to download (e.g. "v2024.1.0"). None = latest.
    pub tag: Option<String>,
    /// Asset filename to pick (e.g. "cloudflared-linux-amd64").
    /// If None, auto-selects based on current OS/arch.
    pub asset: Option<String>,
    /// Optional SHA256 checksum for verification.
    pub checksum: Option<String>,
}

impl GitHubRelease {
    pub fn new(repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            tag: None,
            asset: None,
            checksum: None,
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into()); self
    }

    pub fn asset(mut self, asset: impl Into<String>) -> Self {
        self.asset = Some(asset.into()); self
    }

    pub fn checksum(mut self, sha256: impl Into<String>) -> Self {
        self.checksum = Some(sha256.into()); self
    }
}

#[async_trait]
impl Downloader for GitHubRelease {
    async fn download(&self, dest_dir: &PathBuf) -> Result<DownloadResult, DownloadError> {
        let client = reqwest::Client::new();

        // Resolve tag if not specified.
        let tag = match &self.tag {
            Some(t) => t.clone(),
            None => {
                let resp = client
                    .get(format!("https://api.github.com/repos/{}/releases/latest", self.repo))
                    .header("Accept", "application/vnd.github.v3+json")
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;
                resp["tag_name"].as_str()
                    .ok_or(DownloadError::NotFound("latest release".into()))?
                    .to_string()
            }
        };

        // Fetch release info.
        let release = client
            .get(format!("https://api.github.com/repos/{}/releases/tags/{}", self.repo, tag))
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        // Pick the right asset.
        let assets = release["assets"].as_array()
            .ok_or(DownloadError::NotFound(format!("assets for {}", tag)))?;
        let asset = match &self.asset {
            Some(name) => assets.iter()
                .find(|a| a["name"].as_str() == Some(name))
                .ok_or(DownloadError::NotFound(name.clone()))?,
            None => auto_select_asset(assets)?, // picks based on OS/arch
        };

        let download_url = asset["browser_download_url"].as_str()
            .ok_or(DownloadError::NotFound("download_url".into()))?
            .to_string();

        let file_name = asset["name"].as_str().unwrap();
        let dest_path = dest_dir.join(file_name);

        // Download.
        let bytes = client.get(&download_url).send().await?.bytes().await?;

        // Verify checksum if provided.
        if let Some(expected) = &self.checksum {
            let actual = format!("{:x}", sha2::Sha256::digest(&bytes));
            if actual != expected {
                return Err(DownloadError::ChecksumMismatch {
                    expected: expected.clone(), actual,
                });
            }
        }

        // Write to dest.
        std::fs::create_dir_all(dest_dir)?;
        std::fs::write(&dest_path, &bytes)?;

        // If archive, extract.
        let artifact_path = if file_name.ends_with(".tar.gz") || file_name.ends_with(".zip") {
            extract_archive(&dest_path, dest_dir)?
        } else {
            // Make binary executable.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest_path, perms)?;
            }
            dest_path.clone()
        };

        Ok(DownloadResult {
            artifact_path,
            extra_files: Vec::new(),
            version: tag,
        })
    }
}
```

---

## Part C — Crates.io downloader

For Rust binaries published on crates.io:

```rust
// foundation_downloaders/src/cratesio.rs

pub struct CratesIoBinary {
    /// Crate name (e.g. "cargo-chef").
    pub crate_name: String,
    /// Version to download. None = latest.
    pub version: Option<String>,
    /// Target triple (e.g. "x86_64-unknown-linux-gnu").
    /// None = auto-detect current target.
    pub target: Option<String>,
}

impl CratesIoBinary {
    pub fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            version: None,
            target: None,
        }
    }

    pub fn version(mut self, v: impl Into<String>) -> Self {
        self.version = Some(v.into()); self
    }

    pub fn target(mut self, triple: impl Into<String>) -> Self {
        self.target = Some(triple.into()); self
    }
}

#[async_trait]
impl Downloader for CratesIoBinary {
    async fn download(&self, dest_dir: &PathBuf) -> Result<DownloadResult, DownloadError> {
        let client = reqwest::Client::new();

        // Resolve latest version if not specified.
        let version = match &self.version {
            Some(v) => v.clone(),
            None => {
                let resp = client
                    .get(format!("https://crates.io/api/v1/crates/{}", self.crate_name))
                    .header("User-Agent", "foundation_downloaders/0.0.1")
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;
                resp["crate"]["max_version"].as_str()
                    .ok_or(DownloadError::NotFound(format!("crate: {}", self.crate_name)))?
                    .to_string()
            }
        };

        let target = self.target.as_deref()
            .unwrap_or(std::env::consts::ARCH)
            .to_string();

        // Crates.io doesn't host prebuilt binaries — we use `cargo install`
        // as the download mechanism. For crates that publish prebuilt binaries
        // (e.g. via GitHub releases), use GitHubRelease instead.
        let output = std::process::Command::new("cargo")
            .args([
                "install",
                &self.crate_name,
                "--version", &version,
                "--root",
            ])
            .arg(dest_dir)
            .output()?;

        if !output.status.success() {
            return Err(DownloadError::NotFound(
                format!("cargo install {} failed: {}", self.crate_name,
                    String::from_utf8_lossy(&output.stderr)),
            ));
        }

        let binary_path = dest_dir.join("bin").join(&self.crate_name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&binary_path, perms)?;
        }

        Ok(DownloadResult {
            artifact_path: binary_path,
            extra_files: Vec::new(),
            version,
        })
    }
}
```

---

## Part D — Arbitrary URL downloader

```rust
// foundation_downloaders/src/url.rs

pub struct UrlDownload {
    /// Direct download URL.
    pub url: String,
    /// Expected filename for the downloaded artifact.
    pub filename: Option<String>,
    /// Optional SHA256 checksum.
    pub checksum: Option<String>,
}

impl UrlDownload {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            filename: None,
            checksum: None,
        }
    }

    pub fn filename(mut self, name: impl Into<String>) -> Self {
        self.filename = Some(name.into()); self
    }

    pub fn checksum(mut self, sha256: impl Into<String>) -> Self {
        self.checksum = Some(sha256.into()); self
    }
}

#[async_trait]
impl Downloader for UrlDownload {
    async fn download(&self, dest_dir: &PathBuf) -> Result<DownloadResult, DownloadError> {
        let client = reqwest::Client::new();

        let filename = self.filename.clone()
            .unwrap_or_else(|| {
                // Extract from URL path.
                url::Url::parse(&self.url)
                    .ok()
                    .and_then(|u| u.path_segments())
                    .and_then(|mut s| s.next_back())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("download")
                    .to_string()
            });

        let dest_path = dest_dir.join(&filename);

        let bytes = client.get(&self.url).send().await?.bytes().await?;

        // Verify checksum.
        if let Some(expected) = &self.checksum {
            let actual = format!("{:x}", sha2::Sha256::digest(&bytes));
            if actual != expected {
                return Err(DownloadError::ChecksumMismatch {
                    expected: expected.clone(), actual,
                });
            }
        }

        std::fs::create_dir_all(dest_dir)?;
        std::fs::write(&dest_path, &bytes)?;

        // Make executable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest_path, perms)?;
        }

        Ok(DownloadResult {
            artifact_path: dest_path,
            extra_files: Vec::new(),
            version: filename,
        })
    }
}
```

---

## Part E — DownloadManager (manages + caches downloads)

```rust
// foundation_downloaders/src/manager.rs

/// Download manager — caches downloads by source so they aren't re-downloaded.
pub struct DownloadManager {
    /// Base directory for all downloads.
    base_dir: PathBuf,
    /// Cache: source identifier → cached result.
    cache: Mutex<HashMap<String, DownloadResult>>,
}

impl DownloadManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Download (or return cached) artifact.
    pub async fn download(&self, source: impl Downloader) -> Result<&DownloadResult, DownloadError> {
        // Check cache first.
        let key = source.cache_key();
        {
            let cache = self.cache.lock();
            if let Some(result) = cache.get(&key) {
                return Ok(result);
            }
        }

        // Download into a versioned subdirectory.
        let version_dir = self.base_dir.join(&key);
        let result = source.download(&version_dir).await?;

        // Cache the result.
        let mut cache = self.cache.lock();
        cache.insert(key, result);
        Ok(cache.get(&key).unwrap())
    }

    /// Download multiple artifacts and return paths.
    pub async fn download_all(
        &self,
        sources: Vec<Box<dyn Downloader>>,
    ) -> Result<Vec<DownloadResult>, DownloadError> {
        let mut results = Vec::new();
        for source in sources {
            let result = self.download(source).await?;
            results.push(result.clone());
        }
        Ok(results)
    }
}
```

---

## Part F — Integration with daemon config

Downloaded binary paths feed directly into `DaemonDef`:

```rust
// Example: download then daemon-ify.

let manager = DownloadManager::new(PathBuf::from("/opt/daemons"));

// Download cloudflared from GitHub releases.
let cloudflared = manager.download(
    GitHubRelease::new("cloudflare/cloudflared")
        .checksum("abc123...")
).await?;

// Download a Rust binary from crates.io.
let my_service = manager.download(
    CratesIoBinary::new("my-service")
        .version("1.2.0")
).await?;

// Now configure daemons to keep them running.
let group = DaemonGroup::boot(vec![
    DaemonDef::new("cloudflared", vec![
        cloudflared.artifact_path.to_string_lossy().into_owned(),
        "tunnel", "run", "--token", "xxx",
    ])
    .readiness(ReadinessConfig::Http("http://localhost:9090/ready"))
    .restart(true),
    DaemonDef::new("my-service", vec![
        my_service.artifact_path.to_string_lossy().into_owned(),
        "--config", "/etc/my-service/config.toml",
    ])
    .depends(&["cloudflared"])
    .readiness(ReadinessConfig::Port(8080))
    .memory_limit("500MB"),
]).await?;

// Optionally register for boot-time start.
let registrar = SystemdRegistrar;
registrar.register_protected(BootConfig {
    binary: std::env::current_exe()?,
    args: vec!["--daemon-mode".into()],
    service_name: "my-app".into(),
    privilege: BootPrivilege::User,
    ..Default::default()
})?;
```

### F.2 — Macro integration

The proc macro can reference pre-downloaded binaries:

```rust
#[daemon_process(
    name = "cloudflared",
    download = { github = "cloudflare/cloudflared", checksum = "abc123..." },
    run = ["$DOWNLOAD_DIR/cloudflared", "tunnel", "run", "--token", "xxx"],
    readiness = "port(9090)"
)]
fn main(group: &DaemonGroup) {
    // cloudflared was downloaded and started before this runs.
}
```

The macro expands to:
1. Create/verify `DownloadManager`
2. Download the artifact (or use cached)
3. Substitute `$DOWNLOAD_DIR` in `run` args with the actual path
4. Build `DaemonDef` with resolved path
5. `DaemonGroup::boot(definitions)`
6. Call user function

---

## Verification

```bash
cargo test --package foundation_downloaders -- downloaders
```

Tests cover:
- GitHubRelease: latest tag resolution, asset auto-select, checksum verification
- CratesIoBinary: version resolution, cargo install integration
- UrlDownload: direct URL download, checksum verification
- DownloadManager: cache hit on second download, versioned subdirectories
- Archive extraction: .tar.gz and .zip unpacking
- Integration: downloaded binary path → DaemonDef → boot → running
