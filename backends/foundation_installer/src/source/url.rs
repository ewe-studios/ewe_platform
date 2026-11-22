//! The `url` source: a file at a plain HTTPS URL.

use async_trait::async_trait;

use crate::context::InstallContext;
use crate::error::InstallError;
use crate::manifest::{Entry, SourceSpec};
use crate::source::{download_checked, Artifact, SourceAdapter};

/// The `url` source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlSource;

#[async_trait]
impl SourceAdapter for UrlSource {
    fn kind(&self) -> &'static str {
        "url"
    }

    async fn resolve(&self, entry: &Entry, ctx: &InstallContext) -> Result<Artifact, InstallError> {
        let SourceSpec::Url { url, sha256 } = &entry.source else {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the entry source is not a URL".to_string(),
            });
        };
        if !url.starts_with("https://") {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: format!("the URL {url:?} is not https"),
            });
        }
        download_checked(entry, ctx, url, sha256).await
    }
}
