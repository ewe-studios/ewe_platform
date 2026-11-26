//! The control-plane interface and the sandbox handle.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::shared::command::Command;
use crate::shared::container::ContainerHandle;
use crate::shared::driver::HostReport;
use crate::shared::error::SandboxResult;
use crate::shared::handles::{CreateStream, ExecFileCalls, ExecHandle, FileStream};
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::rpc::messages::SlaveInfo;
use crate::shared::spec::SandboxSpec;
use crate::shared::{BoxByteStream, BoxFuture, ByteStream};

/// The state of one sandbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxState {
    /// The control plane starts the sandbox.
    Creating,
    /// The sandbox is ready for commands and files.
    Ready,
    /// The control plane stops the sandbox.
    Stopping,
    /// The sandbox stopped.
    Stopped,
    /// The sandbox failed.
    Failed,
}

/// What the control plane knows about one sandbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxInfo {
    /// The sandbox identifier.
    pub id: String,
    /// The driver that runs the sandbox.
    pub driver: DriverId,
    /// The state of the sandbox.
    pub state: SandboxState,
    /// The creation time as Unix milliseconds.
    pub created_unix_ms: u64,
    /// What the slave reports, when the control plane reached it.
    pub slave: Option<SlaveInfo>,
}

/// How a sandbox ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitReport {
    /// The exit code of the substrate.
    pub substrate_exit_code: Option<i32>,
    /// Whether the platform forced the exit.
    pub killed: bool,
}

/// The control plane, as a trait.
///
/// `SandboxManager` (local, feature 02) and `SandboxRpcClient` (remote,
/// feature 06) both implement it.
pub trait SandboxProvider: Send + Sync + 'static {
    /// Asks every registered driver what this host can do.
    fn probe(&self) -> BoxFuture<'_, SandboxResult<Vec<HostReport>>>;

    /// Creates one sandbox and returns its handle.
    fn create(&self, spec: SandboxSpec) -> BoxFuture<'_, SandboxResult<SandboxHandle>>;

    /// Creates one sandbox and reports its progress.
    fn create_with_progress(&self, spec: SandboxSpec) -> CreateStream;

    /// Finds a live sandbox.
    fn get(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<SandboxHandle>>;

    /// Lists every live sandbox.
    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SandboxInfo>>>;

    /// Stops one sandbox.
    fn stop(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>>;

    /// Kills one sandbox.
    fn kill(&self, id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>>;
}

/// The operations of one sandbox.
///
/// Feature 02 implements this trait. A caller uses [`SandboxHandle`].
pub trait SandboxOps: Send + Sync + 'static {
    /// What the control plane knows about the sandbox.
    fn info(&self) -> SandboxInfo;

    /// Runs one command.
    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>>;

    /// Writes one file from a byte stream.
    fn write_file(
        &self,
        path: String,
        mode: u32,
        data: BoxByteStream,
    ) -> BoxFuture<'_, SandboxResult<u64>>;

    /// Reads one file as a byte stream.
    fn read_file(&self, path: String) -> BoxFuture<'_, SandboxResult<FileStream>>;

    /// Stops the sandbox.
    fn stop(&self) -> BoxFuture<'_, SandboxResult<ExitReport>>;

    /// Kills the sandbox.
    fn kill(&self) -> BoxFuture<'_, SandboxResult<ExitReport>>;

    /// The container under this sandbox, when the substrate is a container.
    ///
    /// The default is `None` for a microVM.
    fn container(&self) -> Option<ContainerHandle> {
        None
    }

    /// The low-level calls of this sandbox.
    ///
    /// The `ControlService` of feature 06 uses them to pass a remote caller's
    /// calls through with no change.
    fn calls(&self) -> Arc<dyn ExecFileCalls>;
}

/// A cheap handle to one sandbox.
#[derive(Clone)]
pub struct SandboxHandle {
    id: SandboxId,
    ops: Arc<dyn SandboxOps>,
}

impl SandboxHandle {
    /// A handle over a sandbox and its operations.
    #[must_use]
    pub fn new(id: SandboxId, ops: Arc<dyn SandboxOps>) -> Self {
        Self { id, ops }
    }

    /// The sandbox identifier.
    #[must_use]
    pub fn id(&self) -> &SandboxId {
        &self.id
    }

    /// What the control plane knows about the sandbox.
    #[must_use]
    pub fn info(&self) -> SandboxInfo {
        self.ops.info()
    }

    /// The container under this sandbox, when the substrate is a container.
    #[must_use]
    pub fn container(&self) -> Option<ContainerHandle> {
        self.ops.container()
    }

    /// Runs one command.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::SandboxStopped`] when the sandbox
    /// is not ready, or the error of the slave call.
    pub async fn exec(&self, command: Command) -> SandboxResult<ExecHandle> {
        self.ops.exec(command).await
    }

    /// Writes one file from a byte stream and returns its size.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::SandboxStopped`] when the sandbox
    /// is not ready, or the error of the slave call.
    pub async fn write_file(
        &self,
        path: &str,
        mode: u32,
        data: impl ByteStream,
    ) -> SandboxResult<u64> {
        let data: BoxByteStream = Box::pin(data);
        self.ops.write_file(path.to_string(), mode, data).await
    }

    /// Reads one file as a byte stream.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::SandboxStopped`] when the sandbox
    /// is not ready, or the error of the slave call.
    pub async fn read_file(&self, path: &str) -> SandboxResult<FileStream> {
        self.ops.read_file(path.to_string()).await
    }

    /// Stops the sandbox.
    ///
    /// # Errors
    /// The error of the stop call.
    pub async fn stop(&self) -> SandboxResult<ExitReport> {
        self.ops.stop().await
    }

    /// Kills the sandbox.
    ///
    /// # Errors
    /// The error of the kill call.
    pub async fn kill(&self) -> SandboxResult<ExitReport> {
        self.ops.kill().await
    }
}
