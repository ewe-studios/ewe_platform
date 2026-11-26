//! The deployment surface of the sandbox Worker.
//!
//! One [`DeploymentPlan`] describes the Worker, its bindings, its Durable Object
//! (DO) migrations, its containers, its routes and its secret names. Two
//! adapters apply that plan: [`FoundationCloudflareDeployer`] is the primary
//! Rust path, and [`WranglerCloudflareDeployer`] is the supported fallback. See
//! spec 77 feature 08, section 6.8.

pub mod foundation;
pub mod generated_api;
pub mod host;
pub mod plan;
pub mod wrangler;

use std::sync::Arc;

use foundation_context::Ctx;

pub use foundation::FoundationCloudflareDeployer;
pub use generated_api::GeneratedCloudflareApi;
pub use host::{
    CloudflareDeployHost, CommandOutput, CommandRunner, CommandSpec, DeployFuture, DeploymentApi,
    ImageBuilder, SecretStore,
};
pub use plan::{
    validate, Binding, ContainerDeployment, ContainerImage, DeploymentId, DeploymentOutcome,
    DeploymentPlan, DeploymentStatus, DoMigration, Route, SecretRef, SecretSource,
    WorkerDeployment,
};
pub use wrangler::WranglerCloudflareDeployer;

/// One way to fail a deploy.
#[derive(Clone, Debug, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum DeployError {
    /// The `Ctx` has no entry that the deploy needs.
    #[display("the deploy host has no {entry} entry")]
    HostNotConfigured {
        /// The missing entry.
        entry: String,
    },
    /// The plan breaks a rule.
    #[display("the plan is invalid at {field}: {reason}")]
    InvalidPlan {
        /// The field.
        field: String,
        /// Why the field is wrong.
        reason: String,
    },
    /// The platform refused the credentials.
    #[display("authentication failed: {message}")]
    Auth {
        /// The message.
        message: String,
    },
    /// One build step failed.
    #[display("the build step {step} failed (code {code:?}): {message}")]
    Build {
        /// The step.
        step: String,
        /// The exit code, when the step is a program.
        code: Option<i32>,
        /// The message.
        message: String,
    },
    /// The image push failed.
    #[display("the image push of {image} failed: {message}")]
    Push {
        /// The image.
        image: String,
        /// The message.
        message: String,
    },
    /// The API answered an error.
    #[display("the API answered status {status}: {message}")]
    Api {
        /// The HTTP status.
        status: u16,
        /// The message.
        message: String,
    },
    /// Another deploy of the same Worker was in progress.
    #[display("conflict: {message}")]
    Conflict {
        /// The message.
        message: String,
    },
    /// The traffic switch did not finish in time.
    #[display("the rollout of {deployment} timed out")]
    RolloutTimeout {
        /// The deployment.
        deployment: String,
    },
    /// A file or program call failed.
    #[display("I/O failed in {op}: {message}")]
    Io {
        /// The operation.
        op: String,
        /// The message.
        message: String,
    },
    /// The platform cannot do this step.
    #[display("unsupported: {what}")]
    Unsupported {
        /// What the platform cannot do.
        what: String,
    },
}

/// One way to apply a deployment plan.
pub trait CloudflareDeployer: Send + Sync + 'static {
    /// Applies `plan`.
    ///
    /// A failure before the traffic switch leaves the previous Worker live.
    ///
    /// # Errors
    /// [`DeployError`] for the first step that fails.
    fn deploy(
        &self,
        ctx: Ctx,
        plan: DeploymentPlan,
    ) -> DeployFuture<'_, Result<DeploymentOutcome, DeployError>>;

    /// Reads the state of one deployment.
    ///
    /// # Errors
    /// [`DeployError`] when the host cannot read the state.
    fn status(
        &self,
        ctx: Ctx,
        id: &DeploymentId,
    ) -> DeployFuture<'_, Result<DeploymentStatus, DeployError>>;

    /// Restores the previous Worker version and container tags.
    ///
    /// It is idempotent.
    ///
    /// # Errors
    /// [`DeployError`] when the host cannot roll back.
    fn rollback(&self, ctx: Ctx, id: &DeploymentId) -> DeployFuture<'_, Result<(), DeployError>>;
}

/// The deployer of a run that names no adapter: the foundation path.
#[must_use]
pub fn default_deployer() -> Arc<dyn CloudflareDeployer> {
    Arc::new(FoundationCloudflareDeployer::new())
}

/// The deploy host that `ctx` carries.
///
/// # Errors
/// [`DeployError::HostNotConfigured`] when the `Ctx` has no host.
pub(crate) fn host_from(ctx: &Ctx) -> Result<Arc<CloudflareDeployHost>, DeployError> {
    ctx.get::<CloudflareDeployHost>()
        .ok_or_else(|| DeployError::HostNotConfigured {
            entry: "CloudflareDeployHost".to_string(),
        })
}

/// The wall time of `ctx` as Unix milliseconds.
pub(crate) fn deployed_unix_ms(ctx: &Ctx) -> u64 {
    let wall = ctx.clock().wall_now();
    wall.duration_since(foundation_compact::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
