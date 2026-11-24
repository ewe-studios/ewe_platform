//! The `none` deploy target: install only, nothing runs.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::deploy::{DeployTargetAdapter, Deployed};
use crate::error::InstallError;
use crate::manifest::Entry;
use crate::source::Artifact;

/// The `none` deploy adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoneDeploy;

#[async_trait]
impl DeployTargetAdapter for NoneDeploy {
    fn kind(&self) -> &'static str {
        "none"
    }

    async fn apply(
        &self,
        _entry: &Entry,
        artifact: &Artifact,
        _previous: Option<&Deployed>,
        _ctx: &InstallContext,
    ) -> Result<Deployed, InstallError> {
        Ok(Deployed::Installed {
            artifact: artifact.clone(),
        })
    }

    async fn exists(
        &self,
        _entry: &Entry,
        deployed: &Deployed,
        ctx: &InstallContext,
    ) -> Result<bool, InstallError> {
        match deployed {
            Deployed::Installed {
                artifact: Artifact::Image(image),
            } => {
                let docker = ctx.docker()?;
                Ok(docker
                    .image_inspect(&image.repository, &image.tag)
                    .await
                    .is_ok())
            }
            Deployed::Installed { artifact } => Ok(artifact.is_present()),
            _ => Ok(false),
        }
    }
}
