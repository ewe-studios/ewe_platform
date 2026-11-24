//! The `github-release` source: a file attached to a GitHub release.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{Entry, SourceSpec};
use crate::source::{download_checked, encode_path_segment, Artifact, SourceAdapter};

/// The `github-release` source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct GithubReleaseSource;

#[async_trait]
impl SourceAdapter for GithubReleaseSource {
    fn kind(&self) -> &'static str {
        "github-release"
    }

    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError> {
        let SourceSpec::GithubRelease { repo, tag, assets } = &entry.source else {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the entry source is not a GitHub release".to_string(),
            });
        };
        let asset = assets
            .get(ctx.target())
            .ok_or_else(|| InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("the release has no asset for the target {}", ctx.target()),
            })?;
        let (owner, repository) = repo.split_once('/').ok_or_else(|| InstallError::Resolve {
            entry: entry.name.to_string(),
            reason: format!("the repository {repo:?} is not `owner/repo`"),
        })?;
        let url = format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            encode_path_segment(owner),
            encode_path_segment(repository),
            encode_path_segment(tag),
            encode_path_segment(&asset.name),
        );
        download_checked(entry, ctx, &url, &asset.sha256).await
    }
}
