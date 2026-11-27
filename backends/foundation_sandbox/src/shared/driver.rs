//! The driver interface: what a driver can do on this host.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::Stream as ValtronStream;
use serde::{Deserialize, Serialize};

use crate::shared::container::{ContainerControl, ContainerHandle};
use crate::shared::control::SandboxState;
use crate::shared::error::{cut, SandboxError, SandboxResult};
use crate::shared::handles::ExecFileCalls;
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::rpc::messages::SlaveInfo;
use crate::shared::spec::{GuestMount, RootMode, RootSetup, SandboxSpec};
use crate::shared::BoxFuture;

/// What a driver can do on this host.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each capability is an independent yes/no answer that a caller reads one by one"
)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// The host can use a directory as the root.
    pub rootfs_directory: bool,
    /// The host can use an ext4 image as the root.
    pub rootfs_ext4: bool,
    /// The host can use a `SquashFS` image as the root.
    pub rootfs_squashfs: bool,
    /// The host can use a container image as the root.
    pub rootfs_container_image: bool,
    /// The root modes that the host supports.
    pub root_modes: Vec<RootMode>,
    /// The host can mount a host directory.
    pub host_mounts: bool,
    /// The host can run with no network.
    pub network_none: bool,
    /// The host can use the host network.
    pub network_host: bool,
    /// The most virtual CPUs that the host gives to one sandbox.
    pub max_vcpus: u8,
}

/// One step of a create.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateProgress {
    /// The driver prepares the substrate.
    Preparing,
    /// The substrate process or container starts.
    SubstrateStarted,
    /// The driver configures the substrate.
    SubstrateConfigured,
    /// The guest boots.
    Booting,
    /// The control plane reaches the slave.
    SlaveConnected,
    /// The slave is prepared.
    SlaveReady,
}

/// One check of a host probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCheck {
    /// The check name.
    pub name: String,
    /// Whether the check passed.
    pub ok: bool,
    /// The message that explains the result.
    pub message: String,
}

/// What one driver reports about this host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostReport {
    /// The driver.
    pub driver: DriverId,
    /// Whether the driver can run here.
    pub usable: bool,
    /// The checks that the probe ran.
    pub checks: Vec<HostCheck>,
    /// What the driver can do here.
    pub capabilities: Capabilities,
}

impl HostReport {
    /// A clone with every text field cut to `max` bytes.
    pub(crate) fn truncated(&self, max: usize) -> Self {
        Self {
            driver: self.driver,
            usable: self.usable,
            checks: self
                .checks
                .iter()
                .map(|check| HostCheck {
                    name: cut(&check.name, max),
                    ok: check.ok,
                    message: cut(&check.message, max),
                })
                .collect(),
            capabilities: self.capabilities.clone(),
        }
    }
}

/// What a control plane keeps about one sandbox outside its memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredRecord {
    /// The sandbox identifier.
    pub id: SandboxId,
    /// The driver that runs the sandbox.
    pub driver: DriverId,
    /// The spec that the caller gave.
    pub spec: SandboxSpec,
    /// The state of the sandbox.
    pub state: SandboxState,
    /// The creation time as Unix milliseconds.
    pub created_unix_ms: u64,
    /// The lifetime deadline as Unix milliseconds, or `None` for no limit.
    ///
    /// The create writes the value once. A restore reads it and never moves it.
    #[serde(default)]
    pub expires_at_unix_ms: Option<u64>,
    /// How to reach the slave, or `None` while the driver has not answered.
    pub endpoint: Option<SlaveEndpoint>,
    /// The image manifest, when the root came from a named image (feature 12).
    #[serde(default)]
    pub manifest: Option<crate::shared::images::ImageManifest>,
}

/// A started substrate. The slave in it may not answer yet.
pub struct Substrate {
    /// How to reach the slave.
    pub endpoint: SlaveEndpoint,
    /// What the control plane sends in `Prepare`.
    pub root: RootSetup,
    /// The mounts that the control plane sends in `Prepare`.
    pub mounts: Vec<GuestMount>,
    /// The handle of the substrate.
    pub handle: Box<dyn SubstrateHandle>,
    /// The calls of the slave, when the control plane does not dial the slave
    /// itself.
    ///
    /// A nested child forwards each call to the parent's inner control plane
    /// (feature 11). A normal substrate leaves it `None`, and the control plane
    /// dials the slave through the connector.
    pub calls: Option<Arc<dyn ExecFileCalls>>,
    /// What the slave reported, when the driver already reached it.
    ///
    /// A nested child's inner control plane called `Info` and `Prepare`, so the
    /// create task does not call them again (feature 11). A normal substrate
    /// leaves it `None`.
    pub slave: Option<SlaveInfo>,
}

/// How to reach the slave of a substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlaveEndpoint {
    /// A Unix domain socket.
    Unix {
        /// The socket path.
        path: PathBuf,
    },
    /// The Firecracker vsock through a Unix domain socket.
    FirecrackerVsock {
        /// The Unix domain socket path.
        uds: PathBuf,
        /// The vsock port.
        port: u32,
    },
    /// A TCP endpoint.
    Tcp {
        /// The host name or address.
        host: String,
        /// The port.
        port: u16,
    },
    /// A port on a Cloudflare container.
    CloudflarePort {
        /// The port.
        port: u16,
    },
    /// A slave in a second Durable Object, through a service binding.
    CloudflareDo {
        /// The name of the service binding, for example `SLAVE`.
        binding: String,
        /// The name of the slave Durable Object: the sandbox identifier.
        name: SandboxId,
    },
    /// A child sandbox of a parent's inner control plane (feature 11).
    Parent {
        /// The parent sandbox in this control plane.
        parent: SandboxId,
        /// The sandbox inside the parent's inner control plane.
        inner: SandboxId,
    },
}

/// How a substrate ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubstrateExit {
    /// The exit code, when the substrate exited with one.
    pub code: Option<i32>,
    /// The signal, when a signal ended the substrate.
    pub signal: Option<i32>,
    /// Whether the platform forced the exit.
    pub forced: bool,
}

/// The handle of one substrate.
pub trait SubstrateHandle: Send + Sync + 'static {
    /// Completes when the substrate stops by itself.
    fn exited(&self) -> BoxFuture<'_, SubstrateExit>;

    /// Asks for an exit and waits up to `grace`, then forces it.
    fn terminate(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>>;

    /// Forces the substrate to stop.
    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>>;

    /// The last bytes of the substrate log, for error messages.
    fn log_tail(&self, max: usize) -> BoxFuture<'_, Vec<u8>>;

    /// Removes every host resource. It is safe to call this method two times.
    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>>;

    /// The container under this substrate, when the substrate is a container.
    ///
    /// The default is `None` for a microVM or a process.
    fn container(&self) -> Option<ContainerHandle> {
        None
    }

    /// The error for a substrate that stopped during a start.
    ///
    /// The default is [`SandboxError::SubstrateExited`]. A driver whose exit
    /// codes have a meaning gives a more exact error.
    fn exit_error(&self, exit: &SubstrateExit, log_tail: Vec<u8>) -> SandboxError {
        SandboxError::SubstrateExited {
            code: exit.code,
            log_tail,
        }
    }
}

/// The progress stream of one launch.
///
/// `StreamIterator` has a blanket implementation for every `Iterator` whose
/// item is a valtron `Stream`, so this boxed iterator is also a
/// `StreamIterator`.
pub type LaunchStream = Box<
    dyn Iterator<Item = ValtronStream<SandboxResult<Substrate>, CreateProgress>> + Send + 'static,
>;

/// The interface of one sandbox substrate.
pub trait SandboxDriver: Send + Sync + 'static {
    /// The driver identity.
    fn id(&self) -> DriverId;

    /// Checks the host. It starts nothing and ends in `rpc_timeout` or sooner.
    fn probe(&self, ctx: Ctx) -> BoxFuture<'static, HostReport>;

    /// Starts one substrate. It emits progress, then one result.
    ///
    /// The launch gives its result before `deadline`, or fails with
    /// [`SandboxError::BootTimeout`]. The control plane owns the deadline and
    /// uses the same one for the readiness wait and `Prepare`.
    fn launch(&self, ctx: Ctx, id: SandboxId, spec: SandboxSpec, deadline: Instant)
        -> LaunchStream;

    /// Finds the substrate of a stored record again, after a restart.
    ///
    /// `Ok(None)` means that the substrate does not exist any more. A driver
    /// whose substrates die with the control plane's process returns `Ok(None)`
    /// and removes what is left of that sandbox on the host.
    fn attach(
        &self,
        ctx: Ctx,
        record: &StoredRecord,
    ) -> BoxFuture<'_, SandboxResult<Option<Substrate>>>;

    /// The container control of this driver, when it can run a container with
    /// no sandbox.
    ///
    /// The default is `None` for a microVM driver.
    fn containers(&self) -> Option<Arc<dyn ContainerControl>> {
        None
    }
}
