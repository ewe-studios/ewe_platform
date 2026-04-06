//! Deployment target detection and configuration.
//!
//! WHY: The system needs to auto-detect which cloud provider a project targets
//! by examining config files in the project directory.
//!
//! WHAT: `DeploymentTarget` enum representing supported cloud providers, with
//! detection logic based on native config file presence.
//!
//! HOW: Check for `wrangler.toml` (Cloudflare), `service.yaml` (GCP),
//! or `template.yaml` (AWS) in the project root.

use std::path::Path;

/// Supported deployment targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentTarget {
    /// Cloudflare Workers / Pages.
    Cloudflare,
    /// Google Cloud Run.
    GcpCloudRun,
    /// AWS Lambda.
    AwsLambda,
}

impl DeploymentTarget {
    /// Detect provider from project directory by checking for native config files.
    ///
    /// Returns the first matching provider, or `None` if no config file is found.
    #[must_use]
    pub fn detect(project_dir: &Path) -> Option<Self> {
        if project_dir.join("wrangler.toml").exists() {
            Some(Self::Cloudflare)
        } else if project_dir.join("service.yaml").exists() {
            Some(Self::GcpCloudRun)
        } else if project_dir.join("template.yaml").exists() {
            Some(Self::AwsLambda)
        } else {
            None
        }
    }

    /// Returns the native config file name for this provider.
    #[must_use]
    pub fn config_file(self) -> &'static str {
        match self {
            Self::Cloudflare => "wrangler.toml",
            Self::GcpCloudRun => "service.yaml",
            Self::AwsLambda => "template.yaml",
        }
    }

    /// Create a target from a provider name string.
    #[must_use]
    pub fn from_provider_name(name: &str) -> Option<Self> {
        match name {
            "cloudflare" => Some(Self::Cloudflare),
            "gcp" => Some(Self::GcpCloudRun),
            "aws" => Some(Self::AwsLambda),
            _ => None,
        }
    }

    /// Returns the canonical provider name string.
    #[must_use]
    pub fn provider_name(self) -> &'static str {
        match self {
            Self::Cloudflare => "cloudflare",
            Self::GcpCloudRun => "gcp",
            Self::AwsLambda => "aws",
        }
    }
}

impl core::fmt::Display for DeploymentTarget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.provider_name())
    }
}
