//! Cloudflare Workers / Pages deployment provider (CLI-based).
//!
//! WHY: Cloudflare Workers is a primary deployment target for edge applications.
//!
//! WHAT: `CloudflareCliProvider` implements `DeploymentProvider` for Cloudflare Workers,
//! using the `wrangler` CLI for builds and deployments.
//!
//! HOW: Parses `wrangler.toml` for configuration and executes `wrangler` CLI commands.
//! This is a CLI-wrapping provider, not a direct API integration.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::core::shell::{collect_one, ShellDone, ShellExecutor};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildArtifact, BuildOutput, DeploymentResult};
use crate::error::DeploymentError;

/// Cloudflare Workers configuration parsed from `wrangler.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareConfig {
    /// Worker name (required).
    pub name: String,
    /// Account ID (optional, can be set via env).
    pub account_id: Option<String>,
    /// Main entry point for the worker.
    pub main: Option<String>,
    /// Compatibility date (optional).
    pub compatibility_date: Option<String>,
    /// Worker directory (optional, defaults to project root).
    pub workdir: Option<PathBuf>,
}

impl CloudflareConfig {
    /// Parse configuration from a `wrangler.toml` file.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::ConfigInvalid` if the file cannot be read
    /// or parsed.
    pub fn from_file(path: &Path) -> Result<Self, DeploymentError> {
        let content = std::fs::read_to_string(path).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })?;

        toml::from_str(&content).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    }
}

/// Cloudflare Workers / Pages deployment provider (CLI-based).
///
/// This provider wraps the `wrangler` CLI. For direct API integration,
/// a separate `CloudflareApiProvider` could be implemented.
pub struct CloudflareCliProvider;

impl DeploymentProvider for CloudflareCliProvider {
    type Config = CloudflareConfig;
    type Resources = CloudflareResources;

    fn name(&self) -> &str {
        "cloudflare"
    }

    fn detect(project_dir: &Path) -> Option<Self::Config> {
        let config_path = project_dir.join("wrangler.toml");
        if config_path.exists() {
            CloudflareConfig::from_file(&config_path).ok()
        } else {
            None
        }
    }

    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError> {
        if config.name.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "wrangler.toml".to_string(),
                reason: "missing required field: name".to_string(),
            });
        }

        // Check if wrangler is installed
        let check = ShellExecutor::new("wrangler").arg("--version").execute();
        if check.is_err() {
            return Err(DeploymentError::BuildFailed(
                "wrangler CLI not found. Install with `npm install -g wrangler`".to_string(),
            ));
        }

        Ok(())
    }

    fn build(
        &self,
        config: &Self::Config,
        _env: Option<&str>,
    ) -> Result<BuildOutput, DeploymentError> {
        let workdir = config
            .workdir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        // Run wrangler build
        let mut executor = ShellExecutor::new("wrangler")
            .arg("deploy")
            .arg("--dry-run")
            .arg("--outdir")
            .arg(".wrangler-build-output");

        if let Some(main) = &config.main {
            executor = executor.arg(main);
        }

        executor = executor.current_dir(&workdir);

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| DeploymentError::BuildFailed(
            "wrangler build produced no output".to_string(),
        ))?;

        match result {
            ShellDone::Success { stdout, stderr, .. } => {
                tracing::debug!("wrangler build stdout: {}", stdout);
                tracing::debug!("wrangler build stderr: {}", stderr);

                // Check for output files
                let output_dir = workdir.join(".wrangler-build-output");
                let mut artifacts = Vec::new();

                if output_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&output_dir) {
                        for entry in entries.flatten() {
                            if let Ok(meta) = entry.metadata() {
                                artifacts.push(BuildArtifact {
                                    path: entry.path(),
                                    size_bytes: meta.len(),
                                    artifact_type: crate::core::types::ArtifactType::JsBundle,
                                });
                            }
                        }
                    }
                }

                Ok(BuildOutput {
                    artifacts,
                    duration_ms: 0, // TODO: track actual duration
                })
            }
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::BuildFailed(stderr)),
        }
    }

    fn deploy(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        let workdir = config
            .workdir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        let mut executor = ShellExecutor::new("wrangler").arg("deploy");

        if dry_run {
            executor = executor.arg("--dry-run");
        }

        if let Some(env_name) = env {
            executor = executor.arg("--env").arg(env_name);
        }

        executor = executor.current_dir(&workdir);

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| DeploymentError::BuildFailed(
            "wrangler deploy produced no output".to_string(),
        ))?;

        match result {
            ShellDone::Success { stdout, stderr, .. } => {
                tracing::info!("wrangler deploy stdout: {}", stdout);
                tracing::info!("wrangler deploy stderr: {}", stderr);

                // Parse deployment URL from output
                let url = extract_deployed_url(&stdout);

                Ok(DeploymentResult {
                    deployment_id: format!("{}-{}", config.name, Utc::now().timestamp()),
                    provider: "cloudflare".to_string(),
                    resource_name: config.name.clone(),
                    environment: env.map(String::from),
                    url,
                    deployed_at: Utc::now(),
                })
            }
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }

    fn rollback(
        &self,
        _config: &Self::Config,
        _env: Option<&str>,
        _previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError> {
        // Cloudflare doesn't have a direct rollback command
        // In practice, you'd redeploy a previous version from your state store
        Err(DeploymentError::StateFailed(
            "rollback not implemented for Cloudflare - redeploy previous version manually"
                .to_string(),
        ))
    }

    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        let mut executor = ShellExecutor::new("wrangler").arg("tail");

        if let Some(env_name) = env {
            executor = executor.arg("--env").arg(env_name);
        }

        executor = executor.arg(&config.name);

        // Note: tails run forever, so we return immediately after spawning
        // In practice, callers would want to stream the output
        let _ = executor.execute()?;

        Ok(())
    }

    fn destroy(&self, config: &Self::Config, _env: Option<&str>) -> Result<(), DeploymentError> {
        let executor = ShellExecutor::new("wrangler")
            .arg("delete")
            .arg(&config.name);

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| DeploymentError::DeployRejected {
            reason: "wrangler delete produced no output".to_string(),
        })?;

        match result {
            ShellDone::Success { .. } => Ok(()),
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }

    fn status(
        &self,
        _config: &Self::Config,
        _env: Option<&str>,
    ) -> Result<Self::Resources, DeploymentError> {
        // For now, return minimal status - the full implementation would
        // call the Cloudflare API to get deployment info
        Ok(CloudflareResources {
            deployment_id: String::new(),
            version: String::new(),
            deployed_at: None,
            url: None,
        })
    }
}

/// Cloudflare deployment resources/status information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudflareResources {
    /// Deployment identifier.
    pub deployment_id: String,
    /// Worker version (if available).
    pub version: String,
    /// When this version was deployed.
    pub deployed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Deployed URL.
    pub url: Option<String>,
}

/// Extract the deployed URL from wrangler output.
fn extract_deployed_url(output: &str) -> Option<String> {
    // wrangler typically outputs something like:
    // "Published my-worker.abc123.workers.dev"
    for line in output.lines() {
        if let Some(start) = line.find("Published ") {
            let after = &line[start + 10..];
            if let Some(end) = after.find(' ') {
                return Some(after[..end].to_string());
            }
            return Some(after.trim().to_string());
        }
    }
    None
}
