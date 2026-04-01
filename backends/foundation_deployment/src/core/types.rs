//! Shared deployment types used across all providers.
//!
//! WHY: Providers produce common outputs (build artifacts, deployment results,
//! progress events) that the engine and callers consume generically.
//!
//! WHAT: `BuildOutput`, `BuildArtifact`, `ArtifactType`, `DeploymentResult`,
//! and `DeployProgress`.
//!
//! HOW: Plain data structs with `Debug + Clone`. `DeploymentResult` includes
//! factory methods for dry-run and skipped scenarios.

use chrono::{DateTime, Utc};

/// Output from a build step.
#[derive(Debug, Clone)]
pub struct BuildOutput {
    /// Artifacts produced by the build.
    pub artifacts: Vec<BuildArtifact>,
    /// Wall-clock duration of the build in milliseconds.
    pub duration_ms: u64,
}

/// A single build artifact.
#[derive(Debug, Clone)]
pub struct BuildArtifact {
    /// Path to the artifact on disk.
    pub path: std::path::PathBuf,
    /// Size in bytes.
    pub size_bytes: u64,
    /// What kind of artifact this is.
    pub artifact_type: ArtifactType,
}

/// The kind of build artifact produced.
#[derive(Debug, Clone)]
pub enum ArtifactType {
    /// WebAssembly module (`.wasm`).
    WasmModule,
    /// JavaScript bundle.
    JsBundle,
    /// Container image (e.g. Docker).
    ContainerImage {
        /// Image tag (e.g. `gcr.io/project/image:latest`).
        tag: String,
    },
    /// Zip archive (e.g. Lambda deployment package).
    ZipArchive,
    /// Native binary.
    Binary,
}

/// Output from a deployment.
#[derive(Debug, Clone)]
pub struct DeploymentResult {
    /// Provider-assigned deployment identifier.
    pub deployment_id: String,
    /// Provider name ("cloudflare", "gcp", "aws").
    pub provider: String,
    /// Deployed resource name.
    pub resource_name: String,
    /// Target environment, if any.
    pub environment: Option<String>,
    /// Public URL of the deployed service, if available.
    pub url: Option<String>,
    /// Timestamp of the deployment.
    pub deployed_at: DateTime<Utc>,
}

impl DeploymentResult {
    /// Create a result for a dry-run (no actual deployment).
    pub fn dry_run(provider: &str, resource_name: &str) -> Self {
        Self {
            deployment_id: "dry-run".to_string(),
            provider: provider.to_string(),
            resource_name: resource_name.to_string(),
            environment: None,
            url: None,
            deployed_at: Utc::now(),
        }
    }

    /// Create a result for a skipped deployment (config unchanged).
    pub fn skipped(reason: &str) -> Self {
        Self {
            deployment_id: format!("skipped: {reason}"),
            provider: String::new(),
            resource_name: String::new(),
            environment: None,
            url: None,
            deployed_at: Utc::now(),
        }
    }
}

/// Progress events emitted during deployment.
#[derive(Debug, Clone)]
pub enum DeployProgress {
    /// Detecting provider from project directory.
    Detecting,
    /// Validating configuration.
    Validating,
    /// Running build step.
    Building {
        /// Description of the build step.
        step: String,
    },
    /// Packaging build artifacts.
    Packaging,
    /// Uploading artifacts to provider.
    Uploading {
        /// Bytes sent so far.
        bytes_sent: u64,
        /// Total bytes to send.
        total_bytes: u64,
    },
    /// Deploying to provider API.
    Deploying,
    /// Verifying deployment health.
    Verifying,
    /// Deployment completed successfully.
    Complete(DeploymentResult),
    /// Deployment failed.
    Failed(String),
    /// Rolling back a failed deployment.
    RollingBack,
}
