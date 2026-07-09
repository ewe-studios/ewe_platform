//! Image management — pull, build, cache.
//!
//! **WHY:** Images must be present before containers can be created.
//! Users need to define custom Dockerfiles extending base images,
//! with atomic build-or-reuse semantics (build once, cache forever).
//!
//! **WHAT:** `DockerFileConfig` defines a Dockerfile (inline string or
//! file-on-disk) with build context and args. `ImageBuildResult` captures
//! the tagged image and SHA after a build. `build_once()` is the core
//! guarantee — checks if the tag exists, builds only if missing.
//!
//! **HOW:** Uses bollard's `build_image()` (BuildKit) or falls back to
//! `docker build` CLI for complex cases.

use std::path::PathBuf;

/// Builder for a Dockerfile — supports both inline strings
/// (via `include_str!()`) and file-on-disk paths.
#[derive(Debug, Clone)]
pub struct DockerFileConfig {
    /// The Dockerfile content as a string. Use `include_str!("Dockerfile")`
    /// for embedded files, or a dynamically-constructed string.
    pub dockerfile: DockerFileSource,
    /// Build context directory (where COPY/ADD resolve).
    pub context: PathBuf,
    /// Build arguments (--build-arg).
    pub build_args: Vec<(String, String)>,
    /// Tag to assign after build (e.g., "testbed/dev-windows:latest").
    pub tag: String,
    /// Platform (e.g., "linux/amd64"). Auto-detected if `None`.
    pub platform: Option<String>,
}

/// Source of a Dockerfile — either an inline string or a file on disk.
#[derive(Debug, Clone)]
pub enum DockerFileSource {
    /// Dockerfile content as a string literal.
    Inline(String),
    /// Path to a Dockerfile on disk.
    File(PathBuf),
}

/// Result of building a Dockerfile, ready to pipe into
/// `ContainerConfig` via `ContainerConfig::from_build(result)`.
#[derive(Debug, Clone)]
pub struct ImageBuildResult {
    /// The tagged image name (e.g., "testbed/dev-windows:latest").
    pub image_tag: String,
    /// The SHA256 digest of the built image (for pinning).
    pub sha256: String,
    /// Whether the image was built or already existed (cached).
    pub was_cached: bool,
}

impl DockerFileConfig {
    /// Create a new config from an inline Dockerfile string.
    #[must_use]
    pub fn new(dockerfile: impl Into<String>) -> Self {
        Self {
            dockerfile: DockerFileSource::Inline(dockerfile.into()),
            context: PathBuf::from("."),
            build_args: Vec::new(),
            tag: String::new(),
            platform: None,
        }
    }

    /// Create a config pointing to a Dockerfile on disk.
    #[must_use]
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        Self {
            dockerfile: DockerFileSource::File(path.into()),
            context: PathBuf::from("."),
            build_args: Vec::new(),
            tag: String::new(),
            platform: None,
        }
    }

    /// Set a build argument.
    #[must_use]
    pub fn arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.build_args.push((key.into(), value.into()));
        self
    }

    /// Set the build context path.
    #[must_use]
    pub fn context(mut self, path: impl Into<PathBuf>) -> Self {
        self.context = path.into();
        self
    }

    /// Set the image tag.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Set the target platform.
    #[must_use]
    pub fn platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = Some(platform.into());
        self
    }

    /// Build the Dockerfile and tag the result. Returns immediately
    /// if the tag already exists locally (cache hit).
    pub async fn build_once(&self) -> crate::docker::error::DockerResult<ImageBuildResult> {
        // TODO: check if tag exists locally via docker_client.inspect_image
        // If yes → return ImageBuildResult { was_cached: true, ... }
        // If no → build via bollard::build_image, tag, return result
        let _ = self;
        Err(crate::docker::error::DockerError::InvalidConfig(
            "image build not yet implemented".to_string(),
        ))
    }
}
