//! GCP Cloud Run deployment provider (CLI-based).
//!
//! WHY: Google Cloud Run is a primary deployment target for containerized applications.
//!
//! WHAT: `GcpCliProvider` implements `DeploymentProvider` for GCP Cloud Run,
//! using the `gcloud` CLI for builds and deployments.
//!
//! HOW: Parses `service.yaml` for configuration and executes `gcloud` CLI commands.
//! This is a CLI-wrapping provider, not a direct API integration.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::core::shell::{collect_one, ShellDone, ShellExecutor};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildArtifact, BuildOutput, DeploymentResult};
use crate::error::DeploymentError;

/// GCP Cloud Run configuration parsed from `service.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpConfig {
    /// Service name (required).
    pub name: String,
    /// GCP project ID (optional, can be set via env).
    pub project_id: Option<String>,
    /// Deployment region (optional, defaults to us-central1).
    pub region: Option<String>,
    /// Container image tag (optional, auto-generated if not provided).
    pub image: Option<String>,
    /// Service account (optional).
    pub service_account: Option<String>,
}

impl GcpConfig {
    /// Parse configuration from a `service.yaml` file.
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

        serde_yaml::from_str(&content).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    }
}

/// GCP Cloud Run deployment provider (CLI-based).
///
/// This provider wraps the `gcloud` CLI. For direct API integration,
/// a separate `GcpApiProvider` could be implemented.
pub struct GcpCliProvider;

impl DeploymentProvider for GcpCliProvider {
    type Config = GcpConfig;
    type Resources = GcpResources;

    fn name(&self) -> &'static str {
        "gcp"
    }

    fn detect(project_dir: &Path) -> Option<Self::Config> {
        let config_path = project_dir.join("service.yaml");
        if config_path.exists() {
            GcpConfig::from_file(&config_path).ok()
        } else {
            None
        }
    }

    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError> {
        if config.name.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "service.yaml".to_string(),
                reason: "missing required field: name".to_string(),
            });
        }

        // Check if gcloud is installed
        let check = ShellExecutor::new("gcloud").arg("--version").execute();
        if check.is_err() {
            return Err(DeploymentError::BuildFailed(
                "gcloud CLI not found. Install from https://cloud.google.com/sdk".to_string(),
            ));
        }

        Ok(())
    }

    fn build(
        &self,
        config: &Self::Config,
        _env: Option<&str>,
    ) -> Result<BuildOutput, DeploymentError> {
        let region: String = config.region.clone().unwrap_or_else(|| "us-central1".to_string());
        let project_flag = config
            .project_id
            .as_ref()
            .map(|p| format!("--project={p}"))
            .unwrap_or_default();

        // Build and submit container using Cloud Build
        let image = config
            .image
            .clone()
            .unwrap_or_else(|| format!("gcr.io/{project}/{name}:latest", project = config.project_id.as_deref().unwrap_or("PROJECT"), name = config.name));

        let mut executor = ShellExecutor::new("gcloud")
            .arg("builds")
            .arg("submit")
            .arg("--tag")
            .arg(&image)
            .arg("--region")
            .arg(&region);

        if !project_flag.is_empty() {
            executor = executor.arg(project_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("gcloud builds submit produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { stdout, stderr, .. } => {
                tracing::debug!("gcloud build stdout: {stdout}");
                tracing::debug!("gcloud build stderr: {stderr}");

                Ok(BuildOutput {
                    artifacts: vec![BuildArtifact {
                        path: PathBuf::from(&image),
                        size_bytes: 0, // Container image doesn't have a local size
                        artifact_type: crate::core::types::ArtifactType::ContainerImage {
                            tag: image,
                        },
                    }],
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
        let region: String = config.region.clone().unwrap_or_else(|| "us-central1".to_string());
        let project_flag = config
            .project_id
            .as_ref()
            .map(|p| format!("--project={p}"))
            .unwrap_or_default();

        let service_name: String = if let Some(env_name) = env {
            format!("{config_name}-{env_name}", config_name = config.name, env_name = env_name)
        } else {
            config.name.clone()
        };

        let image = config
            .image
            .clone()
            .unwrap_or_else(|| format!("gcr.io/{project}/{name}:latest", project = config.project_id.as_deref().unwrap_or("PROJECT"), name = config.name));

        let mut executor = ShellExecutor::new("gcloud")
            .arg("run")
            .arg("deploy")
            .arg(&service_name)
            .arg("--image")
            .arg(&image)
            .arg("--region")
            .arg(&region)
            .arg("--platform")
            .arg("managed")
            .arg("--allow-unauthenticated");

        if dry_run {
            executor = executor.arg("--dry-run");
        }

        if !project_flag.is_empty() {
            executor = executor.arg(project_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("gcloud run deploy produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { stdout, stderr, .. } => {
                tracing::info!("gcloud deploy stdout: {stdout}");
                tracing::info!("gcloud deploy stderr: {stderr}");

                // Parse deployment URL from output
                let url = extract_deployed_url(&stdout);

                Ok(DeploymentResult {
                    deployment_id: format!("{service_name}-{timestamp}", service_name = service_name, timestamp = Utc::now().timestamp()),
                    provider: "gcp".to_string(),
                    resource_name: service_name,
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
        config: &Self::Config,
        env: Option<&str>,
        previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError> {
        let Some(previous) = previous_state else {
            return Err(DeploymentError::StateFailed(
                "no previous state available for rollback".to_string(),
            ));
        };

        let region: String = config.region.clone().unwrap_or_else(|| "us-central1".to_string());
        let project_flag = config
            .project_id
            .as_ref()
            .map(|p| format!("--project={p}"))
            .unwrap_or_default();

        let service_name: String = if let Some(env_name) = env {
            format!("{config_name}-{env_name}", config_name = config.name, env_name = env_name)
        } else {
            config.name.clone()
        };

        // Deploy the previous image
        let mut executor = ShellExecutor::new("gcloud")
            .arg("run")
            .arg("deploy")
            .arg(&service_name)
            .arg("--image")
            .arg(&previous.image)
            .arg("--region")
            .arg(&region)
            .arg("--platform")
            .arg("managed");

        if !project_flag.is_empty() {
            executor = executor.arg(project_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("gcloud run deploy (rollback) produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { .. } => Ok(()),
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }

    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        let region: String = config.region.clone().unwrap_or_else(|| "us-central1".to_string());

        let service_name: String = if let Some(env_name) = env {
            format!("{config_name}-{env_name}", config_name = config.name, env_name = env_name)
        } else {
            config.name.clone()
        };

        let executor = ShellExecutor::new("gcloud")
            .arg("run")
            .arg("logs")
            .arg("tail")
            .arg(&service_name)
            .arg("--region")
            .arg(&region);

        // Note: logs tail runs forever, so we return immediately after spawning
        let _ = executor.execute()?;

        Ok(())
    }

    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        let region: String = config.region.clone().unwrap_or_else(|| "us-central1".to_string());
        let project_flag = config
            .project_id
            .as_ref()
            .map(|p| format!("--project={p}"))
            .unwrap_or_default();

        let service_name: String = if let Some(env_name) = env {
            format!("{config_name}-{env_name}", config_name = config.name, env_name = env_name)
        } else {
            config.name.clone()
        };

        let mut executor = ShellExecutor::new("gcloud")
            .arg("run")
            .arg("services")
            .arg("delete")
            .arg(&service_name)
            .arg("--region")
            .arg(&region)
            .arg("--quiet");

        if !project_flag.is_empty() {
            executor = executor.arg(project_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::DeployRejected {
                reason: "gcloud run services delete produced no output".to_string(),
            }
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
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<Self::Resources, DeploymentError> {
        let region: String = config.region.clone().unwrap_or_else(|| "us-central1".to_string());

        let service_name: String = if let Some(env_name) = env {
            format!("{config_name}-{env_name}", config_name = config.name, env_name = env_name)
        } else {
            config.name.clone()
        };

        // Get service status
        let mut executor = ShellExecutor::new("gcloud")
            .arg("run")
            .arg("services")
            .arg("describe")
            .arg(&service_name)
            .arg("--region")
            .arg(&region)
            .arg("--format")
            .arg("json");

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("gcloud run services describe produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { stdout, .. } => {
                // Parse JSON output
                let json: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
                    DeploymentError::ConfigInvalid {
                        file: "gcloud output".to_string(),
                        reason: e.to_string(),
                    }
                })?;

                let deployment_id: String = json["metadata"]["generation"]
                    .as_u64()
                    .map(|g| g.to_string())
                    .unwrap_or_default();

                let url = json["status"]["url"]
                    .as_str()
                    .map(String::from);

                let deployed_at = json["metadata"]["creationTimestamp"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                Ok(GcpResources {
                    deployment_id,
                    image: String::new(),
                    deployed_at,
                    url,
                    ready: json["status"]["conditions"]
                        .as_array()
                        .is_some_and(|conditions| {
                            conditions.iter().any(|c| {
                                c["type"].as_str() == Some("Ready")
                                    && c["status"].as_str() == Some("True")
                            })
                        }),
                })
            }
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }
}

/// GCP Cloud Run deployment resources/status information.
#[derive(Debug, Clone)]
pub struct GcpResources {
    /// Deployment identifier (generation number).
    pub deployment_id: String,
    /// Container image that was deployed.
    pub image: String,
    /// When this version was deployed.
    pub deployed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Deployed URL.
    pub url: Option<String>,
    /// Whether the service is ready.
    pub ready: bool,
}

/// Extract the deployed URL from gcloud output.
fn extract_deployed_url(output: &str) -> Option<String> {
    // gcloud run deploy typically outputs something like:
    // "Service [my-service] revision [my-service-00001] has been deployed and is serving at"
    // "URL: https://my-service-abc123.a.run.app"
    for line in output.lines() {
        if line.contains("URL:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].to_string());
            }
        }
    }
    None
}
