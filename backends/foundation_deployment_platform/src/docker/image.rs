//! Image management — pull, build, cache (F04, Decision 05).
//!
//! **WHY:** Tests and providers need images that do not exist on a registry —
//! built from a Dockerfile, from a context on disk or from a string.
//!
//! **WHAT:** [`DockerFileConfig`] describes a build; [`DockerFileConfig::build_once`]
//! runs it and returns an [`ImageBuildResult`], skipping the work when the tag is
//! already present.
//!
//! **HOW:** Through the API clients, never the `docker` CLI — decision 01's whole
//! point is that this stack speaks to the daemon itself (`build_once` used to
//! shell out to `docker build`, which needs the CLI installed, cannot report
//! structured errors, and bypasses the bollard-free client the spec exists for).
//!
//! Two backends, selected by [`BuildBackend`]:
//!
//! - [`BuildBackend::Classic`] (default) — dockerd's own builder over
//!   `POST /build`, with the context uploaded as a tar. The built image lands in
//!   dockerd's image store, so `ContainerConfig::new(tag)` can run it straight
//!   away.
//! - [`BuildBackend::BuildKit`] — a standalone `buildkitd` over gRPC (`Solve`
//!   with the `dockerfile.v0` frontend). Note the image lands in **buildkitd's**
//!   store, not dockerd's: use this to build and export artifacts, not to
//!   produce an image for a local `docker run`.

use std::path::PathBuf;

use foundation_deployment_docker::client::build_context::ContextTar;
use foundation_deployment_docker::client::images::ImageBuildOptions;
use foundation_deployment_docker::DockerClient;

use crate::docker::error::{docker_err, map_docker_client_err, DockerError, DockerResult};

/// Which builder runs the Dockerfile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BuildBackend {
    /// dockerd's classic builder (`POST /build`). The image is left in dockerd's
    /// store, ready to run.
    #[default]
    Classic,

    /// A standalone `buildkitd`, addressed by `host:port` (TCP) or a Unix socket
    /// path. Requires the `buildkit` feature.
    ///
    /// The result lands in buildkitd's store, not dockerd's.
    BuildKit(String),
}

/// Builder for a Dockerfile — inline string or file-on-disk.
#[derive(Debug, Clone)]
pub struct DockerFileConfig {
    pub dockerfile: DockerFileSource,
    pub context: PathBuf,
    pub build_args: Vec<(String, String)>,
    pub tag: String,
    pub platform: Option<String>,
    pub backend: BuildBackend,
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
            backend: BuildBackend::Classic,
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
            backend: BuildBackend::Classic,
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

    /// Choose the builder. Defaults to [`BuildBackend::Classic`].
    #[must_use]
    pub fn backend(mut self, backend: BuildBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Build the Dockerfile, unless `tag` already names a local image.
    ///
    /// # Errors
    /// Returns [`DockerError::InvalidConfig`] if `tag` is empty (there would be
    /// nothing to cache on or run afterwards), [`DockerError::ImageBuild`] if the
    /// build itself fails, or [`DockerError::Connection`] if the daemon is
    /// unreachable.
    pub async fn build_once(&self) -> DockerResult<ImageBuildResult> {
        if self.tag.is_empty() {
            return Err(docker_err(DockerError::InvalidConfig(
                "DockerFileConfig::tag is required — an untagged build cannot be cached or run"
                    .to_string(),
            )));
        }

        match &self.backend {
            BuildBackend::Classic => self.build_classic().await,
            BuildBackend::BuildKit(addr) => self.build_buildkit(addr).await,
        }
    }

    /// dockerd's builder: upload the context as a tar to `POST /build`.
    async fn build_classic(&self) -> DockerResult<ImageBuildResult> {
        let docker = DockerClient::connect_with_defaults()
            .map_err(|e| docker_err(DockerError::Connection(format!("{e}"))))?;

        // Already built? `image_inspect` is the cache check — no CLI, no shelling.
        if let Some(sha) = image_sha(&docker, &self.tag).await? {
            return Ok(ImageBuildResult {
                image_tag: self.tag.clone(),
                sha256: sha,
                was_cached: true,
            });
        }

        let context = self.context_tar()?;

        let outcome = docker
            .image_build(
                &context,
                &ImageBuildOptions {
                    tag: Some(self.tag.clone()),
                    build_args: self.build_args.clone(),
                    platform: self.platform.clone(),
                    ..Default::default()
                },
            )
            .await
            .map_err(map_docker_client_err)?;

        // Prefer the digest the daemon reported; fall back to inspecting the tag.
        let sha256 = match outcome.image_id {
            Some(id) => id,
            None => image_sha(&docker, &self.tag).await?.unwrap_or_default(),
        };

        Ok(ImageBuildResult {
            image_tag: self.tag.clone(),
            sha256,
            was_cached: false,
        })
    }

    /// The build context as a tar: the context directory, with the Dockerfile
    /// written in (inline source) or copied over (file source), so the daemon
    /// always finds it at `Dockerfile`.
    fn context_tar(&self) -> DockerResult<ContextTar> {
        let tar_err = |e: std::io::Error| {
            docker_err(DockerError::InvalidConfig(format!(
                "packing build context {}: {e}",
                self.context.display()
            )))
        };

        match &self.dockerfile {
            DockerFileSource::Inline(content) => {
                if self.context.as_os_str() == "." && !self.context.join("Dockerfile").exists() {
                    // No real context asked for: the Dockerfile is the whole thing.
                    // Avoids tarring up the entire working directory.
                    ContextTar::from_inline_dockerfile("Dockerfile", content).map_err(tar_err)
                } else {
                    ContextTar::from_dir_with_dockerfile(&self.context, "Dockerfile", content)
                        .map_err(tar_err)
                }
            }
            DockerFileSource::File(path) => {
                let content = std::fs::read_to_string(path).map_err(|e| {
                    docker_err(DockerError::InvalidConfig(format!(
                        "reading Dockerfile {}: {e}",
                        path.display()
                    )))
                })?;
                ContextTar::from_dir_with_dockerfile(&self.context, "Dockerfile", &content)
                    .map_err(tar_err)
            }
        }
    }
}

/// The image's id, or `None` if the tag is not present locally.
async fn image_sha(docker: &DockerClient, tag: &str) -> DockerResult<Option<String>> {
    match docker.image_inspect(tag).await {
        Ok(info) => Ok(Some(info.id.unwrap_or_default())),
        Err(foundation_deployment_docker::DockerError::Api { status: 404, .. }) => Ok(None),
        Err(e) => Err(map_docker_client_err(e)),
    }
}

#[cfg(not(feature = "buildkit"))]
impl DockerFileConfig {
    async fn build_buildkit(&self, _addr: &str) -> DockerResult<ImageBuildResult> {
        Err(docker_err(DockerError::InvalidConfig(
            "BuildBackend::BuildKit needs the `buildkit` feature on \
             foundation_deployment_platform"
                .to_string(),
        )))
    }
}

#[cfg(feature = "buildkit")]
impl DockerFileConfig {
    /// buildkitd's builder: serve the context over a session, then `Solve` with
    /// the `dockerfile.v0` frontend.
    async fn build_buildkit(&self, addr: &str) -> DockerResult<ImageBuildResult> {
        use foundation_deployment_docker::buildkit::session::SessionServer;
        use foundation_deployment_docker::buildkit::types::SolveRequest;
        use foundation_deployment_docker::buildkit::{new_build_ref, BuildKitClient};

        // Named fn, not a closure returning a closure: the latter cannot express
        // that `what` outlives the returned mapper.
        fn bk_err(
            what: &'static str,
        ) -> impl FnOnce(
            Box<dyn std::error::Error + Send + Sync>,
        ) -> foundation_errstacks::ErrorTrace<DockerError> {
            move |e| docker_err(DockerError::ImageBuild(format!("buildkit {what}: {e}")))
        }

        let client = BuildKitClient::connect_tcp(addr).map_err(|e| {
            docker_err(DockerError::Connection(format!("buildkitd at {addr}: {e}")))
        })?;

        // buildkitd pulls the context from us over the session — the Dockerfile
        // included, which is why an inline one needs no file on disk.
        let builder = match &self.dockerfile {
            DockerFileSource::Inline(content) => {
                SessionServer::builder_inline(content).map_err(|e| {
                    docker_err(DockerError::InvalidConfig(format!(
                        "staging inline Dockerfile: {e}"
                    )))
                })?
            }
            DockerFileSource::File(_) => SessionServer::builder(&self.context),
        };
        let session = builder
            .start_with(client.transport().clone(), addr)
            .await
            .map_err(bk_err("session"))?;

        let mut frontend_attrs = std::collections::HashMap::new();
        frontend_attrs.insert("filename".to_string(), "Dockerfile".to_string());
        if let Some(ref platform) = self.platform {
            frontend_attrs.insert("platform".to_string(), platform.clone());
        }
        for (k, v) in &self.build_args {
            frontend_attrs.insert(format!("build-arg:{k}"), v.clone());
        }

        let mut exporter_attrs = std::collections::HashMap::new();
        exporter_attrs.insert("name".to_string(), self.tag.clone());

        let request = SolveRequest {
            Ref: new_build_ref(),
            Session: session.id.clone(),
            Frontend: "dockerfile.v0".to_string(),
            FrontendAttrs: frontend_attrs.into_iter().collect(),
            ExporterDeprecated: "image".to_string(),
            ExporterAttrsDeprecated: exporter_attrs.into_iter().collect(),
            ..Default::default()
        };

        let response = client.solve(request).await.map_err(bk_err("solve"))?;

        // buildkitd returns the built image's digest here; there is no dockerd
        // image to inspect, since the result lives in buildkitd's store.
        let sha256 = response
            .ExporterResponse
            .get("containerimage.digest")
            .cloned()
            .unwrap_or_default();

        Ok(ImageBuildResult {
            image_tag: self.tag.clone(),
            sha256,
            was_cached: false,
        })
    }
}
