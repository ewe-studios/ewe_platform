//! Discovery: find one manifest entry on the host.
//!
//! WHY: the installer marks each unit and container (spec 79 feature 00), so
//! the sidecar finds an entry by its mark without reading the installer's
//! private record (spec 82 feature 01, section 4.2).
//!
//! WHAT: [`Discovery`], [`Discovered`], [`RunHandle`], the small
//! [`CommandRunner`] trait and [`DiscoverySet`], which picks the adapter for an
//! entry's deploy kind.
//!
//! HOW: the systemd adapter runs `systemctl show`; the Docker adapter uses the
//! generated `DockerClient` calls. Both take their side effects from
//! [`RegistrarContext`](crate::RegistrarContext).

pub mod docker;
pub mod systemd;

use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::RegistrarError;
use crate::manifest::EntryView;
use crate::RegistrarContext;

/// What one process run wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// The exit status code.
    pub status: i32,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
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
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
#[display("cannot run {program}: {reason}")]
pub struct CommandError {
    /// The program.
    pub program: String,
    /// The reason from the operating system.
    pub reason: String,
}

impl std::error::Error for CommandError {}

/// Runs one program with an argument list.
///
/// The sidecar passes the program and its arguments as separate items, never
/// through a shell. A test replaces this trait to fake `systemctl`.
#[async_trait]
pub trait CommandRunner: Send + Sync {
    /// Run `program` with `args` and collect its output.
    ///
    /// # Errors
    /// [`CommandError`] when the program cannot be started.
    async fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput, CommandError>;
}

/// The host process runner.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandRunner;

#[async_trait]
impl CommandRunner for SystemCommandRunner {
    async fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput, CommandError> {
        // A process wait is a boundary call: this is the host implementation.
        let output = std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|error| CommandError {
                program: program.to_string(),
                reason: error.to_string(),
            })?;
        Ok(CommandOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Finds one entry on the host. One implementation for each deploy kind.
#[async_trait]
pub trait Discovery: Send + Sync {
    /// The deploy kind that this adapter finds.
    fn kind(&self) -> &'static str;

    /// Find `entry`.
    ///
    /// # Errors
    /// [`RegistrarError::Discovery`] when the host cannot be asked.
    async fn discover(
        &self,
        entry: &EntryView,
        ctx: &RegistrarContext,
    ) -> Result<Discovered, RegistrarError>;
}

/// What discovery found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovered {
    /// No unit or container with this entry's mark.
    NotInstalled,
    /// Installed, not running (unit inactive or failed, container exited).
    Stopped {
        /// The version mark that the installer wrote.
        version: String,
    },
    /// Running.
    Running {
        /// The version mark that the installer wrote.
        version: String,
        /// The process or the container.
        handle: RunHandle,
        /// When the app started (Unix milliseconds).
        started_at_unix_ms: u64,
        /// Restarts that systemd or Docker counted.
        restarts: u32,
        /// Docker only: the container IP on its network.
        container_ip: Option<IpAddr>,
    },
}

impl Discovered {
    /// The running version, or the version mark of a stopped app.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::NotInstalled => None,
            Self::Stopped { version } | Self::Running { version, .. } => Some(version),
        }
    }

    /// The run handle of a running app.
    #[must_use]
    pub fn handle(&self) -> Option<&RunHandle> {
        match self {
            Self::Running { handle, .. } => Some(handle),
            _ => None,
        }
    }

    /// True when the app runs.
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// The identity of one running app.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RunHandle {
    /// A systemd unit and its main process.
    Process {
        /// The main process ID.
        pid: u32,
        /// The unit name, for example `ewe-agent-rpc.service`.
        unit: String,
    },
    /// A Docker container.
    Container {
        /// The container ID.
        id: String,
    },
}

/// The adapters that the reconciler calls, chosen by `deploy.kind`.
pub struct DiscoverySet {
    systemd: Arc<dyn Discovery>,
    docker: Arc<dyn Discovery>,
}

impl std::fmt::Debug for DiscoverySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoverySet")
            .field("systemd", &self.systemd.kind())
            .field("docker", &self.docker.kind())
            .finish()
    }
}

impl DiscoverySet {
    /// A set over the two adapters.
    #[must_use]
    pub fn new(systemd: Arc<dyn Discovery>, docker: Arc<dyn Discovery>) -> Self {
        Self { systemd, docker }
    }
}

#[async_trait]
impl Discovery for DiscoverySet {
    fn kind(&self) -> &'static str {
        "set"
    }

    async fn discover(
        &self,
        entry: &EntryView,
        ctx: &RegistrarContext,
    ) -> Result<Discovered, RegistrarError> {
        match entry.deploy.kind.as_str() {
            "systemd" => self.systemd.discover(entry, ctx).await,
            "docker" => self.docker.discover(entry, ctx).await,
            // `none` runs nothing, so the sidecar does not watch it.
            "none" => Ok(Discovered::NotInstalled),
            other => Err(RegistrarError::Discovery {
                entry: entry.name.clone(),
                reason: format!("unknown deploy kind {other:?}"),
            }),
        }
    }
}
