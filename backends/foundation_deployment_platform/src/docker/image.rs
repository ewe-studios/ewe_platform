//! Image management — pull, build, cache.

use std::path::PathBuf;
use std::process::Command;
use foundation_deployment_docker::DockerClient;
use crate::docker::error::{docker_err, DockerError, DockerResult};

/// Builder for a Dockerfile — inline string or file-on-disk.
#[derive(Debug, Clone)]
pub struct DockerFileConfig {
    pub dockerfile: DockerFileSource,
    pub context: PathBuf,
    pub build_args: Vec<(String, String)>,
    pub tag: String,
    pub platform: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DockerFileSource {
    Inline(String),
    File(PathBuf),
}

/// Result of building a Dockerfile.
#[derive(Debug, Clone)]
pub struct ImageBuildResult {
    pub image_tag: String,
    pub sha256: String,
    pub was_cached: bool,
}

impl DockerFileConfig {
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

    #[must_use]
    pub fn arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.build_args.push((key.into(), value.into()));
        self
    }

    #[must_use]
    pub fn context(mut self, path: impl Into<PathBuf>) -> Self {
        self.context = path.into();
        self
    }

    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    #[must_use]
    pub fn platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = Some(platform.into());
        self
    }

    /// Build the Dockerfile. Returns immediately if the tag already
    /// exists locally (checked via `docker image inspect`).
    pub async fn build_once(&self) -> DockerResult<ImageBuildResult> {
        // Check cache: does the tag already exist?
        if let Ok(true) = image_exists_locally(&self.tag).await {
            let sha = get_image_sha(&self.tag).await.unwrap_or_default();
            return Ok(ImageBuildResult {
                image_tag: self.tag.clone(),
                sha256: sha,
                was_cached: true,
            });
        }

        // Write Dockerfile to temp file (inline or copy from source)
        let dockerfile_path = match &self.dockerfile {
            DockerFileSource::Inline(content) => {
                let tmp = std::env::temp_dir().join(format!("Dockerfile.{}", std::process::id()));
                std::fs::write(&tmp, content)
                    .map_err(|e| docker_err(DockerError::InvalidConfig(format!("write Dockerfile: {e}"))))?;
                tmp
            }
            DockerFileSource::File(path) => path.clone(),
        };

        // Build via docker CLI
        let mut cmd = Command::new("docker");
        cmd.arg("build")
            .arg("-t").arg(&self.tag)
            .arg("-f").arg(&dockerfile_path);

        for (k, v) in &self.build_args {
            cmd.arg("--build-arg").arg(format!("{k}={v}"));
        }
        if let Some(ref platform) = self.platform {
            cmd.arg("--platform").arg(platform);
        }
        cmd.arg(self.context.as_os_str());

        let output = cmd.output()
            .map_err(|e| docker_err(DockerError::InvalidConfig(format!("docker build: {e}"))))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(docker_err(DockerError::InvalidConfig(format!(
                "docker build failed: {stderr}"
            ))));
        }

        // Clean up temp file
        if matches!(self.dockerfile, DockerFileSource::Inline(_)) {
            let _ = std::fs::remove_file(&dockerfile_path);
        }

        let sha = get_image_sha(&self.tag).await.unwrap_or_default();
        Ok(ImageBuildResult {
            image_tag: self.tag.clone(),
            sha256: sha,
            was_cached: false,
        })
    }
}

/// Check if an image exists locally via the Docker API.
async fn image_exists_locally(tag: &str) -> Result<bool, DockerError> {
    let client = DockerClient::connect_with_defaults()
        .map_err(|e| DockerError::Connection(format!("{e}")))?;
    match client.image_inspect(tag).await {
        Ok(_) => Ok(true),
        Err(foundation_deployment_docker::DockerError::Api { status: 404, .. }) => Ok(false),
        Err(e) => Err(DockerError::Connection(format!("inspect_image: {e}"))),
    }
}

/// Get the SHA256 digest of a locally available image.
async fn get_image_sha(tag: &str) -> Result<String, DockerError> {
    let client = DockerClient::connect_with_defaults()
        .map_err(|e| DockerError::Connection(format!("{e}")))?;
    let info = client.image_inspect(tag).await
        .map_err(|e| DockerError::Connection(format!("inspect_image: {e}")))?;
    Ok(info.id.unwrap_or_default())
}
