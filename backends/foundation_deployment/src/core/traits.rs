//! The `DeploymentProvider` trait — the central abstraction for cloud providers.
//!
//! WHY: The deployment engine needs to call build, deploy, rollback, and verify
//! operations without knowing which cloud it's talking to.
//!
//! WHAT: A trait with associated `Config` and `Resources` types, covering the
//! full deployment lifecycle: detect, validate, build, deploy, rollback, logs,
//! destroy, status, and verify.
//!
//! HOW: Each cloud provider (Cloudflare, GCP, AWS) implements this trait.
//! The engine calls methods generically via `P: DeploymentProvider`.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::core::types::{BuildOutput, DeploymentResult};
use crate::error::DeploymentError;

/// Every cloud deployment target implements this trait.
///
/// The engine calls these methods without knowing which cloud it's talking to.
/// `Config` holds provider-specific settings (account ID, region, worker name, etc.),
/// and `Resources` describes the provider's view of what's deployed.
pub trait DeploymentProvider {
    /// Provider-specific configuration type (parsed from native config file).
    type Config: DeserializeOwned + Serialize + Clone + core::fmt::Debug;

    /// Provider-specific resource information returned by `status()`.
    type Resources: core::fmt::Debug;

    /// Human-readable provider name ("cloudflare", "gcp", "aws").
    fn name(&self) -> &str;

    /// Try to detect this provider's config file in `project_dir`.
    ///
    /// Returns `Some(config)` if a recognizable config file is found and
    /// parseable, `None` otherwise.
    fn detect(project_dir: &Path) -> Option<Self::Config>
    where
        Self: Sized;

    /// Validate the configuration without deploying.
    ///
    /// Checks that required fields are present, credentials are available, etc.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::ConfigInvalid` if configuration is invalid,
    /// or `DeploymentError::BuildFailed` if required tools are missing.
    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError>;

    /// Build the project artifacts.
    ///
    /// `env` selects a named environment (staging, production, etc.).
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::BuildFailed` if the build process fails,
    /// or `DeploymentError::ProcessFailed` if a required command fails.
    fn build(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<BuildOutput, DeploymentError>;

    /// Deploy to the target.
    ///
    /// If `dry_run` is true, validate and build but don't push to the provider.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::DeployRejected` if the provider rejects the deployment,
    /// or `DeploymentError::BuildFailed` if building artifacts fails.
    fn deploy(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError>;

    /// Rollback a failed deployment.
    ///
    /// Called when `deploy()` fails after partially completing.
    /// The provider determines how to revert to the previous known-good state.
    ///
    /// `previous_state` contains the last known-good deployment state
    /// from the state store, if one exists.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::StateFailed` if rollback is not supported,
    /// or `DeploymentError::DeployRejected` if the provider rejects the rollback.
    fn rollback(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError>;

    /// Tail logs from the deployed service.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::ProcessFailed` if the log streaming command fails.
    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError>;

    /// Tear down deployed resources.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::DeployRejected` if the provider rejects the deletion.
    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError>;

    /// Query current status of deployed resources.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::HttpError` if the provider API request fails,
    /// or `DeploymentError::StateFailed` if state retrieval fails.
    fn status(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<Self::Resources, DeploymentError>;

    /// Verify a deployment is healthy and responsive.
    ///
    /// Called after deploy to confirm the deployment succeeded.
    /// Returns `Ok(true)` if healthy, `Ok(false)` if not yet healthy
    /// (may be retried), or `Err(e)` if verification itself fails.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::HttpError` if the health check request fails.
    fn verify(&self, result: &DeploymentResult) -> Result<bool, DeploymentError> {
        let _ = result;
        Ok(true)
    }
}
