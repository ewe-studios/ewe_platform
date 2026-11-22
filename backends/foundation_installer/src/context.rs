//! The context that an adapter uses for every side effect.
//!
//! No adapter reads a global (root rule 11). The HTTP client, the Docker API,
//! the process runner, the clock and the target triple come from here. A test
//! puts fakes in the same fields, so the adapter code under test does not
//! change.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use foundation_context::{Clock, Ctx};
use foundation_deployment_docker::client::ContainerCreateBody;
use foundation_deployment_docker::generated::create::ContainerCreateResponse;
use foundation_deployment_docker::generated::json::{ContainerInspectResponse, ImageInspect};
use foundation_deployment_docker::{DockerClient, DockerError};
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::HttpClientBuilder;

use crate::config::InstallerConfig;
use crate::error::InstallError;

/// The outcome of one process run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// The exit status code.
    pub status: i32,
    /// What the program wrote to standard output.
    pub stdout: String,
    /// What the program wrote to standard error.
    pub stderr: String,
}

impl CommandOutput {
    /// True when the program exited with code 0.
    #[must_use]
    pub fn success(&self) -> bool {
        self.status == 0
    }
}

/// A process could not be run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot run {program}: {reason}")]
pub struct CommandError {
    /// The program that the caller tried to run.
    pub program: String,
    /// The reason from the operating system.
    pub reason: String,
}

/// Runs one program with an argument list.
///
/// The installer passes package names and arguments as separate items, never
/// through a shell (spec 79 section 4.11). A test replaces this trait to fake
/// `cargo`, `apt-get`, `mise` and `systemctl`.
#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// Run `program` with `args` and collect its output.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when the program cannot be started.
    async fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput, CommandError>;
}

/// The host process runner.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandRunner;

#[async_trait]
impl CommandRunner for SystemCommandRunner {
    async fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput, CommandError> {
        // A process wait is a boundary call: this is the host implementation,
        // not core logic. Adapters reach it only through this trait.
        let output = std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|err| CommandError {
                program: program.to_string(),
                reason: err.to_string(),
            })?;
        Ok(CommandOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// The Docker operations that the installer uses.
///
/// The host implementation, [`DockerEngine`], calls
/// `foundation_deployment_docker` with its generated types. Tests replace this
/// trait with a fake, so the deploy and source adapters run with no daemon.
#[async_trait]
pub trait DockerApi: Send + Sync {
    /// Pull `repository:tag` from its registry and read the progress to the end.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the daemon refuses the pull.
    async fn image_pull(&self, repository: &str, tag: &str) -> Result<(), DockerError>;

    /// Inspect an image by `repository:tag`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the image is absent or the daemon fails.
    async fn image_inspect(&self, repository: &str, tag: &str)
        -> Result<ImageInspect, DockerError>;

    /// Create a container with `name`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the daemon refuses the create.
    async fn create_container(
        &self,
        body: &ContainerCreateBody,
        name: &str,
    ) -> Result<ContainerCreateResponse, DockerError>;

    /// Start a container.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the daemon refuses the start.
    async fn start_container(&self, id: &str) -> Result<(), DockerError>;

    /// Inspect a container.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the container is absent or the daemon
    /// fails.
    async fn inspect_container(&self, id: &str) -> Result<ContainerInspectResponse, DockerError>;

    /// Stop a container and wait the given time before Docker kills it.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the daemon fails.
    async fn stop_container(&self, id: &str, timeout_secs: Option<u32>) -> Result<(), DockerError>;

    /// Remove a container.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the daemon fails.
    async fn remove_container(&self, id: &str, force: bool) -> Result<(), DockerError>;

    /// Rename a container.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the name is in use or the daemon fails.
    async fn rename_container(&self, id: &str, name: &str) -> Result<(), DockerError>;
}

/// The host Docker API, over the local daemon socket.
#[derive(Debug, Clone)]
pub struct DockerEngine {
    client: DockerClient,
}

impl DockerEngine {
    /// Wrap an existing client.
    #[must_use]
    pub fn new(client: DockerClient) -> Self {
        Self { client }
    }

    /// Connect to the daemon that the environment names, or the default Unix
    /// socket.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] when the socket or the connection settings are
    /// not usable.
    pub fn connect_default() -> Result<Self, DockerError> {
        Ok(Self {
            client: DockerClient::connect_with_defaults()?,
        })
    }
}

#[async_trait]
impl DockerApi for DockerEngine {
    async fn image_pull(&self, repository: &str, tag: &str) -> Result<(), DockerError> {
        self.client
            .image_pull(None, None, Some(repository), Some(tag), None, None)
            .await?
            .finish()
            .await
    }

    async fn image_inspect(
        &self,
        repository: &str,
        tag: &str,
    ) -> Result<ImageInspect, DockerError> {
        self.client
            .image_inspect(&format!("{repository}:{tag}"))
            .await
    }

    async fn create_container(
        &self,
        body: &ContainerCreateBody,
        name: &str,
    ) -> Result<ContainerCreateResponse, DockerError> {
        self.client.create_container(body, Some(name)).await
    }

    async fn start_container(&self, id: &str) -> Result<(), DockerError> {
        self.client.start_container(id).await
    }

    async fn inspect_container(&self, id: &str) -> Result<ContainerInspectResponse, DockerError> {
        self.client.inspect_container(id).await
    }

    async fn stop_container(&self, id: &str, timeout_secs: Option<u32>) -> Result<(), DockerError> {
        self.client.stop_container(id, timeout_secs).await
    }

    async fn remove_container(&self, id: &str, force: bool) -> Result<(), DockerError> {
        self.client.remove_container(id, force).await
    }

    async fn rename_container(&self, id: &str, name: &str) -> Result<(), DockerError> {
        self.client.rename_container(id, name).await
    }
}

/// Everything that an adapter or the installer needs from its host.
#[derive(Clone)]
pub struct InstallContext {
    config: InstallerConfig,
    ctx: Ctx,
    http: Option<Arc<dyn HttpClient>>,
    docker: Option<Arc<dyn DockerApi>>,
    commands: Option<Arc<dyn CommandRunner>>,
    target: String,
}

impl InstallContext {
    /// A context with the given configuration and clock, and no side effects.
    /// Add the ones that the run needs with the `with_*` methods.
    #[must_use]
    pub fn new(config: InstallerConfig, ctx: Ctx) -> Self {
        Self {
            config,
            ctx,
            http: None,
            docker: None,
            commands: None,
            target: env!("FOUNDATION_INSTALLER_TARGET").to_string(),
        }
    }

    /// Add the HTTP client.
    #[must_use]
    pub fn with_http(mut self, http: Arc<dyn HttpClient>) -> Self {
        self.http = Some(http);
        self
    }

    /// Add the Docker API.
    #[must_use]
    pub fn with_docker(mut self, docker: Arc<dyn DockerApi>) -> Self {
        self.docker = Some(docker);
        self
    }

    /// Add the process runner.
    #[must_use]
    pub fn with_commands(mut self, commands: Arc<dyn CommandRunner>) -> Self {
        self.commands = Some(commands);
        self
    }

    /// Replace the target triple that a GitHub release asset is chosen by.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = target.into();
        self
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &InstallerConfig {
        &self.config
    }

    /// The engine context, for the clock and the random numbers.
    #[must_use]
    pub fn context(&self) -> &Ctx {
        &self.ctx
    }

    /// The clock that the engine seeded.
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.ctx.clock()
    }

    /// The current Unix time in seconds.
    #[must_use]
    pub fn unix_seconds(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.clock().wall_now(),
        ))
        .unwrap_or_default()
    }

    /// Wait for `duration` on the engine clock.
    pub async fn sleep(&self, duration: Duration) {
        foundation_core::valtron::sleep_async(duration, &self.ctx).await;
    }

    /// The HTTP client.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::MissingSideEffect`] when the context has none.
    pub fn http(&self) -> Result<&Arc<dyn HttpClient>, InstallError> {
        self.http
            .as_ref()
            .ok_or_else(|| InstallError::MissingSideEffect("the HTTP client".to_string()))
    }

    /// The Docker API.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::MissingSideEffect`] when the context has none.
    pub fn docker(&self) -> Result<&Arc<dyn DockerApi>, InstallError> {
        self.docker
            .as_ref()
            .ok_or_else(|| InstallError::MissingSideEffect("the Docker API".to_string()))
    }

    /// The process runner.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::MissingSideEffect`] when the context has none.
    pub fn commands(&self) -> Result<&Arc<dyn CommandRunner>, InstallError> {
        self.commands
            .as_ref()
            .ok_or_else(|| InstallError::MissingSideEffect("the process runner".to_string()))
    }

    /// The target triple that this build was made for.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// A context for a host: the real HTTP client, the real Docker daemon and
    /// the real process runner. Only the binary calls this.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::MissingSideEffect`] when the Docker client
    /// cannot be built from the environment.
    pub fn host(config: InstallerConfig) -> Result<Self, InstallError> {
        let ctx = Ctx::try_host()
            .map_err(|err| InstallError::MissingSideEffect(format!("the entropy source: {err}")))?;
        let http: Arc<dyn HttpClient> = HttpClientBuilder::new()
            .connect_timeout(config.connect_timeout)
            .read_timeout(config.read_timeout)
            .build();
        let docker = DockerEngine::connect_default()
            .map_err(|err| InstallError::MissingSideEffect(format!("the Docker client: {err}")))?;
        Ok(Self::new(config, ctx)
            .with_http(http)
            .with_docker(Arc::new(docker))
            .with_commands(Arc::new(SystemCommandRunner)))
    }
}
