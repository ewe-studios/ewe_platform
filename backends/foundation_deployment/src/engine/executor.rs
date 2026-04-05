//! Deployment executor.
//!
//! WHY: High-level API for running deployment state machines to completion.
//!
//! WHAT: `DeploymentExecutor` provides `deploy()`, `destroy()`, and `status()` methods.
//!
//! HOW: Wraps `StateMachineTask` and Valtron's `execute()` to drive deployments.

use crate::config::DeploymentTarget;
use crate::core::traits::DeploymentProvider;
use crate::core::types::DeploymentResult;
use crate::error::DeploymentError;
use crate::providers::cloudflare::provider::CloudflareCliProvider;
use foundation_core::valtron::{execute, StateMachineTask, Stream};
use foundation_db::state::StateStore;

use super::planner::{DeployOutcome, DeploymentPlanner};

/// High-level API that creates a planner and runs it to completion.
pub struct DeploymentExecutor;

impl DeploymentExecutor {
    /// Deploy a project. Auto-detects provider, loads config, runs state machine.
    ///
    /// **Caller requirement**: A valtron `PoolGuard` must be alive in the calling
    /// scope. Binary entry points initialize it; library code must not.
    /// ```rust,no_run
    /// // In main() or test setup:
    /// let _guard = foundation_core::valtron::executors::unified::initialize_pool(42, None);
    /// let result = DeploymentExecutor::deploy(project_dir, None, false, state_store);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Directory containing the project to deploy
    /// * `env` - Optional environment name (e.g., "staging", "production")
    /// * `dry_run` - If true, simulate deployment without applying
    /// * `state_store` - State store for persisting deployment state
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError` if provider detection, validation, build, deploy,
    /// or verification fails.
    pub fn deploy(
        project_dir: &std::path::Path,
        env: Option<&str>,
        dry_run: bool,
        state_store: Box<dyn StateStore>,
    ) -> Result<DeploymentResult, DeploymentError> {
        let target = DeploymentTarget::detect(project_dir)
            .ok_or_else(|| DeploymentError::NoProviderDetected {
                project_dir: project_dir.display().to_string(),
            })?;

        match target {
            DeploymentTarget::Cloudflare => {
                let config = CloudflareCliProvider::detect(project_dir)
                    .ok_or_else(|| DeploymentError::ConfigInvalid {
                        file: "wrangler.toml".into(),
                        reason: "failed to parse".into(),
                    })?;
                let provider = CloudflareCliProvider;
                let planner = DeploymentPlanner::new(provider, config, state_store)
                    .environment_opt(env)
                    .dry_run(dry_run);
                run_state_machine(planner)
            }
            DeploymentTarget::GcpCloudRun => {
                // TODO: Implement GCP provider integration
                Err(DeploymentError::Generic(
                    "GCP Cloud Run provider not yet implemented in executor".into()
                ))
            }
            DeploymentTarget::AwsLambda => {
                // TODO: Implement AWS provider integration
                Err(DeploymentError::Generic(
                    "AWS Lambda provider not yet implemented in executor".into()
                ))
            }
        }
    }

    /// Destroy resources for a project.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Directory containing the project
    /// * `env` - Optional environment name
    /// * `state_store` - State store for reading deployment state
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError` if provider detection fails or destroy operation fails.
    pub fn destroy(
        project_dir: &std::path::Path,
        env: Option<&str>,
        _state_store: Box<dyn StateStore>,
    ) -> Result<(), DeploymentError> {
        let target = DeploymentTarget::detect(project_dir)
            .ok_or_else(|| DeploymentError::NoProviderDetected {
                project_dir: project_dir.display().to_string(),
            })?;

        match target {
            DeploymentTarget::Cloudflare => {
                let config = CloudflareCliProvider::detect(project_dir)
                    .ok_or_else(|| DeploymentError::ConfigInvalid {
                        file: "wrangler.toml".into(),
                        reason: "failed to parse".into(),
                    })?;
                let provider = CloudflareCliProvider;
                provider.destroy(&config, env)
            }
            DeploymentTarget::GcpCloudRun | DeploymentTarget::AwsLambda => {
                Err(DeploymentError::Generic(
                    "Provider destroy not yet implemented".into()
                ))
            }
        }
    }

    /// Show deployment status.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Directory containing the project
    /// * `env` - Optional environment name
    /// * `state_store` - State store for reading deployment state
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError` if provider detection fails or status query fails.
    pub fn status(
        project_dir: &std::path::Path,
        env: Option<&str>,
        _state_store: Box<dyn StateStore>,
    ) -> Result<serde_json::Value, DeploymentError> {
        let target = DeploymentTarget::detect(project_dir)
            .ok_or_else(|| DeploymentError::NoProviderDetected {
                project_dir: project_dir.display().to_string(),
            })?;

        match target {
            DeploymentTarget::Cloudflare => {
                let config = CloudflareCliProvider::detect(project_dir)
                    .ok_or_else(|| DeploymentError::ConfigInvalid {
                        file: "wrangler.toml".into(),
                        reason: "failed to parse".into(),
                    })?;
                let provider = CloudflareCliProvider;
                let resources = provider.status(&config, env)?;
                Ok(serde_json::to_value(resources)?)
            }
            DeploymentTarget::GcpCloudRun | DeploymentTarget::AwsLambda => {
                Err(DeploymentError::Generic(
                    "Provider status not yet implemented".into()
                ))
            }
        }
    }
}

/// Run a valtron `StateMachine` to completion, collecting the final result.
///
/// The `Output` type is `DeployOutcome` (`Result<DeploymentResult, DeploymentError>`),
/// so errors are delivered as `Stream::Next(Err(e))` — never swallowed.
///
/// # Type bounds
///
/// The generic `M` must implement `StateMachine` with:
/// - `Output = DeployOutcome`
/// - `Error = DeploymentError`
/// - `Action = NoAction`
/// - `Send + 'static` for executor compatibility
/// - `State: Send` for thread safety
fn run_state_machine<M>(machine: M) -> Result<DeploymentResult, DeploymentError>
where
    M: foundation_core::valtron::StateMachine<
            Output = DeployOutcome,
            Error = DeploymentError,
            Action = foundation_core::valtron::NoAction,
        > + Send + 'static,
    M::State: Send,
{
    let task = StateMachineTask::new(machine);
    let mut stream = execute(task, None)
        .map_err(|e| DeploymentError::ExecutorError { reason: e.to_string() })?;

    // Drive the stream to completion. The last Stream::Next value is the outcome.
    let mut last_outcome: Option<DeployOutcome> = None;
    loop {
        match stream.next() {
            Some(Stream::Next(outcome)) => {
                last_outcome = Some(outcome);
            }
            Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Ignore) => {}
            Some(Stream::Init) => {}
            None => break,
        }
    }

    last_outcome.unwrap_or_else(|| Err(DeploymentError::ExecutorError {
        reason: "state machine produced no output".to_string(),
    }))
}

// Helper method is already defined in DeploymentPlanner, no need to re-implement here
