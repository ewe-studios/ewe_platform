//! The `docker-image` source: pull the image and record its digest.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{Entry, SourceSpec};
use crate::source::{Artifact, ImageRef, SourceAdapter};

/// The `docker-image` source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct DockerImageSource;

#[async_trait]
impl SourceAdapter for DockerImageSource {
    fn kind(&self) -> &'static str {
        "docker-image"
    }

    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError> {
        let SourceSpec::DockerImage {
            repository,
            tag,
            digest,
        } = &entry.source
        else {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the entry source is not a container image".to_string(),
            });
        };
        let docker = ctx.docker()?.clone();
        docker
            .image_pull(repository, tag)
            .await
            .map_err(|err| InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: err.to_string(),
            })?;
        let inspect =
            docker
                .image_inspect(repository, tag)
                .await
                .map_err(|err| InstallError::Resolve {
                    entry: entry.name.to_string(),
                    reason: err.to_string(),
                })?;
        let actual = inspect
            .repo_digests
            .as_ref()
            .and_then(|digests| {
                digests
                    .iter()
                    .find_map(|digest| digest.split_once('@').map(|(_, value)| value.to_string()))
            })
            .ok_or_else(|| InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("Docker reported no repository digest for {repository}:{tag}"),
            })?;
        if let Some(expected) = digest {
            let expected = normalize_digest(expected);
            if actual != expected {
                return Err(InstallError::ChecksumMismatch {
                    entry: entry.name.to_string(),
                    expected,
                    actual,
                });
            }
        }
        Ok(Artifact::Image(ImageRef {
            repository: repository.clone(),
            tag: tag.clone(),
            digest: actual,
        }))
    }
}

/// Put the `sha256:` prefix on a digest that has none.
#[must_use]
pub fn normalize_digest(digest: &str) -> String {
    if digest.starts_with("sha256:") {
        digest.to_string()
    } else {
        format!("sha256:{digest}")
    }
}
