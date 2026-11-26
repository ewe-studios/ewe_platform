//! The injected side effects of a deployment.
//!
//! The deployers read a [`CloudflareDeployHost`] from the `Ctx`. The host holds
//! the Cloudflare application programming interface (API) client, the image
//! builder, the command runner and the secret store. Tests give fakes, so no
//! network call and no container engine run.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use foundation_auth::server_credential::Secret;

use crate::deploy::plan::{
    Binding, ContainerDeployment, DoMigration, Route, SecretSource, WorkerDeployment,
};
use crate::deploy::plan::{DeploymentId, DeploymentStatus};
use crate::deploy::DeployError;

/// A boxed future that is `Send`.
pub type DeployFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The Cloudflare API operations of one deploy.
pub trait DeploymentApi: Send + Sync + 'static {
    /// Creates or updates the container application and the container class of
    /// the Worker.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn ensure_container(
        &self,
        worker_name: &str,
        container: &ContainerDeployment,
        tag: &str,
    ) -> DeployFuture<'_, Result<(), DeployError>>;

    /// Uploads a new Worker version. The version is not live yet.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn upload_version(
        &self,
        worker: &WorkerDeployment,
        bindings: &[Binding],
        migrations: &[DoMigration],
    ) -> DeployFuture<'_, Result<String, DeployError>>;

    /// Writes one secret on one Worker version.
    ///
    /// The value is never logged.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn put_secret(
        &self,
        worker_name: &str,
        version: &str,
        name: &str,
        value: &Secret,
    ) -> DeployFuture<'_, Result<(), DeployError>>;

    /// Sets one route.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn set_route(
        &self,
        worker_name: &str,
        route: &Route,
    ) -> DeployFuture<'_, Result<(), DeployError>>;

    /// Promotes a version to serve traffic.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn promote(
        &self,
        worker_name: &str,
        version: &str,
    ) -> DeployFuture<'_, Result<String, DeployError>>;

    /// Reads the state of one deployment.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn status(&self, id: &DeploymentId) -> DeployFuture<'_, Result<DeploymentStatus, DeployError>>;

    /// Restores the previous Worker version and container tags.
    ///
    /// # Errors
    /// [`DeployError::Api`] when the platform refuses the call.
    fn rollback(&self, id: &DeploymentId) -> DeployFuture<'_, Result<(), DeployError>>;
}

/// Builds and pushes one container image.
pub trait ImageBuilder: Send + Sync + 'static {
    /// Builds an image for `linux/amd64` from `dockerfile` and tags it `tag`.
    ///
    /// # Errors
    /// [`DeployError::Build`] when the build fails.
    fn build(&self, dockerfile: &Path, tag: &str) -> DeployFuture<'_, Result<(), DeployError>>;

    /// Pushes `tag` to its registry.
    ///
    /// # Errors
    /// [`DeployError::Push`] when the push fails.
    fn push(&self, tag: &str) -> DeployFuture<'_, Result<(), DeployError>>;
}

/// One command to run.
///
/// The standard input carries a secret value, so this type has no `Debug`.
pub struct CommandSpec {
    /// The program.
    pub program: String,
    /// The arguments.
    pub args: Vec<String>,
    /// The working directory.
    pub cwd: Option<PathBuf>,
    /// The text for the standard input.
    pub stdin: Option<Secret>,
}

/// What one command printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
    /// The exit code.
    pub code: i32,
    /// The standard output.
    pub stdout: String,
    /// The standard error.
    pub stderr: String,
}

/// Runs one program.
pub trait CommandRunner: Send + Sync + 'static {
    /// Runs `spec` to its end.
    ///
    /// # Errors
    /// [`DeployError::Io`] when the program cannot run.
    fn run(&self, spec: CommandSpec) -> DeployFuture<'_, Result<CommandOutput, DeployError>>;
}

/// Reads one secret value from its source.
pub trait SecretStore: Send + Sync + 'static {
    /// Reads the value of `name` from `source`.
    ///
    /// # Errors
    /// [`DeployError::Auth`] when the secret is absent or the store refuses the
    /// read.
    fn get(
        &self,
        name: &str,
        source: &SecretSource,
    ) -> DeployFuture<'_, Result<Secret, DeployError>>;
}

/// The side effects of one deployment.
///
/// The foundation adapter needs `api`, `images`, `commands` and `secrets`. The
/// `wrangler` adapter needs `commands` and `secrets`. A missing entry is
/// [`DeployError::HostNotConfigured`].
#[derive(Clone, Default)]
pub struct CloudflareDeployHost {
    /// The Cloudflare API client.
    pub api: Option<Arc<dyn DeploymentApi>>,
    /// The container image builder.
    pub images: Option<Arc<dyn ImageBuilder>>,
    /// The command runner.
    pub commands: Option<Arc<dyn CommandRunner>>,
    /// The secret store.
    pub secrets: Option<Arc<dyn SecretStore>>,
}

impl CloudflareDeployHost {
    /// A host with no entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same host with an API client.
    #[must_use]
    pub fn with_api(mut self, api: Arc<dyn DeploymentApi>) -> Self {
        self.api = Some(api);
        self
    }

    /// The same host with an image builder.
    #[must_use]
    pub fn with_images(mut self, images: Arc<dyn ImageBuilder>) -> Self {
        self.images = Some(images);
        self
    }

    /// The same host with a command runner.
    #[must_use]
    pub fn with_commands(mut self, commands: Arc<dyn CommandRunner>) -> Self {
        self.commands = Some(commands);
        self
    }

    /// The same host with a secret store.
    #[must_use]
    pub fn with_secrets(mut self, secrets: Arc<dyn SecretStore>) -> Self {
        self.secrets = Some(secrets);
        self
    }
}

impl std::fmt::Debug for CloudflareDeployHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CloudflareDeployHost")
            .field("api", &self.api.is_some())
            .field("images", &self.images.is_some())
            .field("commands", &self.commands.is_some())
            .field("secrets", &self.secrets.is_some())
            .finish()
    }
}

/// The host entry of one type, or [`DeployError::HostNotConfigured`].
pub(crate) fn require<T: ?Sized>(entry: Option<Arc<T>>, name: &str) -> Result<Arc<T>, DeployError> {
    entry.ok_or_else(|| DeployError::HostNotConfigured {
        entry: name.to_string(),
    })
}
