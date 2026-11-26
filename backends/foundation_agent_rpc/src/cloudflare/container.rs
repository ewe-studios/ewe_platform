//! The native agent-rpc container over the spec 77 container handle
//! (spec 83 feature 06, T6).
//!
//! WHY: the full model server (`foundation_agent_rpc`, the native binary) runs
//! in a Cloudflare container with enough memory and central processing unit
//! (CPU) time for llama.cpp. The Worker and the Durable Object (DO) never call
//! the Cloudflare container application programming interface (API) directly.
//! They hold the spec 77 [`ContainerHandle`] and call it.
//!
//! WHAT: [`AgentContainer`] (the live container), [`ContainerOptions`] (the
//! spec of a start) and [`AGENT_RPC_PORT`] (the port that the native server
//! listens on).
//!
//! HOW: `start` builds a [`ContainerSpec`] and passes it to
//! [`ContainerControl::start`]. `call` sends one Hypertext Transfer Protocol
//! (HTTP) request to a port in the container; the response body stays a
//! stream. `exec` reads the handle's [`ContainerCapabilities`] first, so a
//! runtime without the capability answers
//! [`SandboxError::UnsupportedCapability`] and no exec call reaches the
//! container. `stop` waits at most the grace period.

use std::time::Duration;

use foundation_context::Ctx;
use foundation_sandbox::shared::cloudflare::CloudflarePlatform;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::container::{
    ContainerCapabilities, ContainerControl, ContainerHandle, ContainerId, ContainerResources,
    ContainerSpec, PortRequest, PortResponse,
};
use foundation_sandbox::shared::driver::SubstrateExit;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::handles::ExecHandle;
use foundation_sandbox::shared::spec::NetworkMode;

/// The port of the Connect and gRPC-Web server in the native agent-rpc image.
///
/// The image starts `agent-slave` as process identifier 1 (PID 1), and the
/// slave starts the native agent-rpc server on this port.
pub const AGENT_RPC_PORT: u16 = 8_080;

/// The image entry point of the agent-rpc container.
///
/// `agent-slave` is PID 1 in the image; it starts the native agent-rpc server.
/// An empty list keeps the image default, so this feature never guesses the
/// image command.
pub const AGENT_RPC_ENTRYPOINT: &[&str] = &[];

/// What a caller may change in a container start.
///
/// Every value has a documented default, so `start` needs no options
/// (root rule 8: limits are configuration).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerOptions {
    /// The entry point. Empty keeps the image default.
    pub entrypoint: Vec<String>,
    /// The container environment. No secret comes from the image.
    pub env: Vec<(String, String)>,
    /// The network mode. Default [`NetworkMode::Host`]: the model server may
    /// reach a remote model provider.
    pub network: NetworkMode,
    /// The resources. Cloudflare reads `instance_type`; it ignores `vcpus`
    /// and `memory_mib` because the instance type sets both.
    pub resources: ContainerResources,
    /// The container labels. Cloudflare ignores them.
    pub labels: Vec<(String, String)>,
}

impl Default for ContainerOptions {
    fn default() -> Self {
        Self {
            entrypoint: AGENT_RPC_ENTRYPOINT
                .iter()
                .map(ToString::to_string)
                .collect(),
            env: Vec::new(),
            network: NetworkMode::Host,
            resources: ContainerResources::default(),
            labels: Vec::new(),
        }
    }
}

impl ContainerOptions {
    /// The spec that [`ContainerControl::start`] reads.
    #[must_use]
    pub fn spec(&self, image: String) -> ContainerSpec {
        ContainerSpec {
            image,
            entrypoint: self.entrypoint.clone(),
            env: self.env.clone(),
            network: self.network,
            resources: self.resources.clone(),
            labels: self.labels.clone(),
        }
    }
}

/// The native agent-rpc container of one Durable Object.
#[derive(Clone)]
pub struct AgentContainer {
    /// The spec 77 handle of the live container.
    pub handle: ContainerHandle,
}

impl std::fmt::Debug for AgentContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentContainer")
            .field("id", self.handle.id())
            .finish_non_exhaustive()
    }
}

impl AgentContainer {
    /// Start the native agent-rpc container through the Cloudflare wrapper of
    /// spec 77.
    ///
    /// The control comes from `cf.control`; the container is started with the
    /// context of `cf.ctx`.
    ///
    /// # Errors
    /// The [`SandboxError`] of the start: a wrong image, a refused start or a
    /// missing capability.
    pub async fn start(cf: &CloudflarePlatform, image: String) -> SandboxResult<Self> {
        Self::start_with(
            cf.control.as_ref(),
            cf.ctx.clone(),
            image,
            ContainerOptions::default(),
        )
        .await
    }

    /// Start the container with options.
    ///
    /// # Errors
    /// The [`SandboxError`] of the start.
    pub async fn start_with_options(
        cf: &CloudflarePlatform,
        image: String,
        options: ContainerOptions,
    ) -> SandboxResult<Self> {
        Self::start_with(cf.control.as_ref(), cf.ctx.clone(), image, options).await
    }

    /// Start the container through any spec 77 container control.
    ///
    /// A caller uses this method when it holds a control that is not the
    /// Cloudflare one, for example a test double.
    ///
    /// # Errors
    /// The [`SandboxError`] of the start.
    pub async fn start_with(
        control: &dyn ContainerControl,
        ctx: Ctx,
        image: String,
        options: ContainerOptions,
    ) -> SandboxResult<Self> {
        let handle = control.start(ctx, options.spec(image)).await?;
        Ok(Self { handle })
    }

    /// The container identifier.
    #[must_use]
    pub fn id(&self) -> &ContainerId {
        self.handle.id()
    }

    /// What the runtime can do.
    #[must_use]
    pub fn capabilities(&self) -> ContainerCapabilities {
        self.handle.capabilities()
    }

    /// Send one Connect request to `port` in the native server.
    ///
    /// The response body is a stream. This method never reads the whole body.
    ///
    /// # Errors
    /// [`SandboxError::UnsupportedCapability`] when the runtime cannot fetch a
    /// port, or [`SandboxError::Rpc`] when the port does not answer.
    pub async fn call(&self, port: u16, request: PortRequest) -> SandboxResult<PortResponse> {
        self.handle.fetch(port, request).await
    }

    /// Run one command in the container.
    ///
    /// The method reads [`ContainerCapabilities::exec`] first. When the
    /// runtime cannot run a command it answers
    /// `UnsupportedCapability { capability: "exec" }` and no exec call reaches
    /// the container.
    ///
    /// # Errors
    /// [`SandboxError::UnsupportedCapability`] when the runtime has no exec
    /// capability, or the error of the exec call.
    pub async fn exec(&self, command: Command) -> SandboxResult<ExecHandle> {
        if !self.handle.capabilities().exec {
            return Err(SandboxError::UnsupportedCapability {
                capability: "exec".to_string(),
            }
            .into());
        }
        self.handle.exec(command).await
    }

    /// Stop the container with a grace period, then force it.
    ///
    /// # Errors
    /// The error of the runtime call.
    pub async fn stop(&self, grace: Duration) -> SandboxResult<SubstrateExit> {
        self.handle.stop(grace).await
    }

    /// Remove the container and every resource it holds.
    ///
    /// It is safe to call this method two times.
    ///
    /// # Errors
    /// The error of the runtime call.
    pub async fn destroy(&self) -> SandboxResult<()> {
        self.handle.destroy().await
    }
}
