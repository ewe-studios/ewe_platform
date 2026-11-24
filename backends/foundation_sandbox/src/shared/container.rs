//! A container runtime that the platform can drive.
//!
//! A container is the substrate of a sandbox, and it is also a first-class
//! object. [`ContainerControl`] is the factory and [`ContainerHandle`] is the
//! live object.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use crate::shared::command::Command;
use crate::shared::driver::SubstrateExit;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::ExecHandle;
use crate::shared::ids::DriverId;
use crate::shared::spec::NetworkMode;
use crate::shared::{BoxByteStream, BoxFuture};

/// A container identifier.
///
/// The value is 1 to 64 characters of `[A-Za-z0-9_.-]`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContainerId(String);

impl ContainerId {
    /// A container identifier from a value.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] when the value is empty, is longer than 64
    /// characters, or holds a character outside `[A-Za-z0-9_.-]`.
    pub fn new(value: String) -> SandboxResult<Self> {
        let shape_is_valid = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
        if !shape_is_valid {
            return Err(SandboxError::InvalidSpec {
                field: "container_id".to_string(),
                reason: "must be 1 to 64 characters of [A-Za-z0-9_.-]".to_string(),
            }
            .into());
        }
        Ok(Self(value))
    }

    /// The identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ContainerId {
    type Err = foundation_errstacks::ErrorTrace<SandboxError>;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.to_string())
    }
}

impl TryFrom<String> for ContainerId {
    type Error = foundation_errstacks::ErrorTrace<SandboxError>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ContainerId> for String {
    fn from(value: ContainerId) -> Self {
        value.0
    }
}

/// The state of one container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerStatus {
    /// The container is starting.
    Starting,
    /// The container is running.
    Running,
    /// The container stopped.
    Stopped,
    /// The container failed.
    Failed,
}

/// The resources of one container.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerResources {
    /// The number of virtual CPUs. Docker only.
    pub vcpus: Option<u8>,
    /// The memory in mebibytes. Docker only.
    pub memory_mib: Option<u32>,
    /// The instance type. Cloudflare only; `lite` is the minimum.
    pub instance_type: Option<String>,
}

/// What a caller wants a container to be.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// The Docker tag, or the image name that the Worker config sets.
    pub image: String,
    /// The entrypoint. Empty means the image default.
    pub entrypoint: Vec<String>,
    /// The container environment. No secret is ever put in the image.
    pub env: Vec<(String, String)>,
    /// The network mode. [`NetworkMode::Host`] enables the container network.
    pub network: NetworkMode,
    /// The resources of the container.
    pub resources: ContainerResources,
    /// The container labels.
    pub labels: Vec<(String, String)>,
}

/// What one container control knows about a container.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerInfo {
    /// The container identifier.
    pub id: ContainerId,
    /// The driver that runs the container.
    pub driver: DriverId,
    /// The image name.
    pub image: String,
    /// The state of the container.
    pub status: ContainerStatus,
    /// The creation time as Unix milliseconds.
    pub created_unix_ms: u64,
    /// The resources of the container.
    pub resources: ContainerResources,
}

/// What a container runtime can do.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each capability is an independent yes/no answer that a caller reads one by one"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerCapabilities {
    /// The runtime can run a command.
    pub exec: bool,
    /// The runtime can send an HTTP request to a port.
    pub port_fetch: bool,
    /// The runtime can signal the container.
    pub signal: bool,
    /// The runtime can read the container log.
    pub logs: bool,
}

/// A container runtime that the platform can drive.
pub trait ContainerControl: Send + Sync + 'static {
    /// Starts one container and returns its handle.
    ///
    /// The handle owns the container until `destroy`.
    fn start(&self, ctx: Ctx, spec: ContainerSpec)
        -> BoxFuture<'_, SandboxResult<ContainerHandle>>;

    /// Finds a container that this control started, after a restart.
    ///
    /// It never starts a container.
    fn get(
        &self,
        ctx: Ctx,
        id: &ContainerId,
    ) -> BoxFuture<'_, SandboxResult<Option<ContainerHandle>>>;

    /// Lists the containers that this control knows.
    fn list(&self, ctx: Ctx) -> BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>>;
}

/// A cheap handle to one container.
#[derive(Clone)]
pub struct ContainerHandle {
    id: ContainerId,
    ops: Arc<dyn ContainerOps>,
}

impl ContainerHandle {
    /// A handle over a container and its operations.
    #[must_use]
    pub fn new(id: ContainerId, ops: Arc<dyn ContainerOps>) -> Self {
        Self { id, ops }
    }

    /// The container identifier.
    #[must_use]
    pub fn id(&self) -> &ContainerId {
        &self.id
    }

    /// What this control knows about the container.
    #[must_use]
    pub fn info(&self) -> ContainerInfo {
        self.ops.info()
    }

    /// What the runtime can do.
    #[must_use]
    pub fn capabilities(&self) -> ContainerCapabilities {
        self.ops.capabilities()
    }

    /// Sends one HTTP request to a port in the container. The body streams.
    ///
    /// # Errors
    /// [`SandboxError::UnsupportedCapability`] when the runtime cannot fetch a
    /// port, or [`SandboxError::Rpc`] when the port does not answer.
    pub async fn fetch(&self, port: u16, request: PortRequest) -> SandboxResult<PortResponse> {
        self.ops.fetch(port, request).await
    }

    /// Runs one command in the container.
    ///
    /// # Errors
    /// [`SandboxError::UnsupportedCapability`] when the runtime cannot run a
    /// command.
    pub async fn exec(&self, command: Command) -> SandboxResult<ExecHandle> {
        self.ops.exec(command).await
    }

    /// Sends a signal to the container.
    ///
    /// # Errors
    /// [`SandboxError::UnsupportedCapability`] when the runtime cannot signal.
    pub async fn signal(&self, signo: i32) -> SandboxResult<()> {
        self.ops.signal(signo).await
    }

    /// Completes when the container exits by itself.
    ///
    /// # Errors
    /// The error of the runtime call.
    pub async fn wait(&self) -> SandboxResult<SubstrateExit> {
        self.ops.wait().await
    }

    /// Stops the container with a grace period, then forces it.
    ///
    /// # Errors
    /// The error of the runtime call.
    pub async fn stop(&self, grace: Duration) -> SandboxResult<SubstrateExit> {
        self.ops.stop(grace).await
    }

    /// Removes the container and every resource it holds.
    ///
    /// It is safe to call this method two times.
    ///
    /// # Errors
    /// The error of the runtime call.
    pub async fn destroy(&self) -> SandboxResult<()> {
        self.ops.destroy().await
    }

    /// The last bytes of the container log.
    ///
    /// # Errors
    /// The error of the runtime call.
    pub async fn log_tail(&self, max: usize) -> SandboxResult<Vec<u8>> {
        self.ops.log_tail(max).await
    }
}

/// The operations of one container.
///
/// A runtime implements this trait. A caller uses [`ContainerHandle`].
pub trait ContainerOps: Send + Sync + 'static {
    /// What this control knows about the container.
    fn info(&self) -> ContainerInfo;

    /// What the runtime can do.
    fn capabilities(&self) -> ContainerCapabilities;

    /// Sends one HTTP request to a port in the container.
    fn fetch(&self, port: u16, request: PortRequest) -> BoxFuture<'_, SandboxResult<PortResponse>>;

    /// Runs one command in the container.
    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>>;

    /// Sends a signal to the container.
    fn signal(&self, signo: i32) -> BoxFuture<'_, SandboxResult<()>>;

    /// Completes when the container exits by itself.
    fn wait(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>>;

    /// Stops the container with a grace period, then forces it.
    fn stop(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>>;

    /// Removes the container and every resource it holds.
    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>>;

    /// The last bytes of the container log.
    fn log_tail(&self, max: usize) -> BoxFuture<'_, SandboxResult<Vec<u8>>>;
}

/// One HTTP request to a port in a container.
///
/// The request body is one small message of at most `max_message_bytes`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortRequest {
    /// The HTTP method.
    pub method: String,
    /// The request path.
    pub path: String,
    /// The request headers.
    pub headers: Vec<(String, String)>,
    /// The request body.
    #[serde(with = "crate::shared::serde_helpers::base64_bytes")]
    pub body: Vec<u8>,
}

/// One HTTP response from a port in a container.
///
/// The body streams.
pub struct PortResponse {
    /// The HTTP status.
    pub status: u16,
    /// The response headers.
    pub headers: Vec<(String, String)>,
    /// The response body.
    pub body: BoxByteStream,
}

/// The error of an adapter's private port call.
#[derive(Clone, Debug, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum PortError {
    /// No server listens on the port.
    #[display("no server listens on the port")]
    NotListening,
    /// Another platform error. The text is cut to `max_error_message`.
    #[display("port error: {_0}")]
    Other(#[error(not(source))] String),
}
