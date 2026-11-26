//! The control role of the slave (feature 11, section 6.1).
//!
//! A slave that runs in a container can also serve `SandboxService`: it holds
//! the same [`SandboxManager`] and [`FirecrackerDriver`] code that a host
//! control plane holds. The outer control plane then creates micro virtual
//! machines (VMs) in that container through the slave.
//!
//! This module builds the inner control plane. It adds no sandbox rule: the
//! state machine, the drivers and the record store are those of the control
//! plane (invariant I5). A build with no `control` or `firecracker` feature has
//! no such role; the slave then reports `can_control: false`.
//!
//! A Cloudflare container has no `/dev/kvm` (feature 11, section 6.6). The same
//! code runs there, the Firecracker probe reports the missing device, and the
//! slave reports `can_control: false`. A caller that creates a child then gets
//! `DriverUnavailable` (invariant I2). No code change is needed if Cloudflare
//! adds the capability.

use std::sync::Arc;

use foundation_context::Ctx;
use foundation_core::valtron::block_on_future;

use crate::native::firecracker::driver::FirecrackerDriver;
use crate::native::host::NativeConnector;
use crate::shared::control::{ManagerConfig, MemoryRecordStore, SandboxManager, SandboxProvider};
use crate::shared::driver::SandboxDriver;
use crate::shared::error::SandboxResult;
use crate::shared::host::{HostConfig, SandboxHost};
use crate::shared::ids::DriverId;

/// The inner control plane of one slave.
pub struct ControlRole {
    /// The manager that serves `SandboxService`.
    pub provider: Arc<dyn SandboxProvider>,
    /// Whether the Firecracker probe of this container is usable.
    pub can_control: bool,
}

/// Builds the inner control plane from the container environment.
///
/// `HostConfig::discover` reads `EWE_FIRECRACKER`, `EWE_SANDBOX_SLAVE` and the
/// cache directory, which the Docker driver sets at container create (feature
/// 11, section 6.3).
///
/// # Errors
/// [`crate::shared::error::SandboxError::HostNotConfigured`] when the context
/// has no environment, or the error of the program path.
pub fn build(ctx: &Ctx) -> SandboxResult<ControlRole> {
    let executable =
        std::env::current_exe().map_err(|error| crate::shared::error::SandboxError::Io {
            op: "control".to_string(),
            message: format!("the program path is not known: {error}"),
        })?;
    let directory = executable
        .parent()
        .ok_or_else(|| crate::shared::error::SandboxError::Io {
            op: "control".to_string(),
            message: "the program directory is not known".to_string(),
        })?;
    let mut config = HostConfig::discover(|name| std::env::var(name).ok(), directory);
    // A nested host runs micro virtual machines: the inner order is Firecracker
    // only, so a missing KVM device is a clear probe result and not a fallback.
    config.driver_order = vec![DriverId::Firecracker];
    ctx.store(SandboxHost::native(config));

    let driver: Arc<dyn SandboxDriver> = Arc::new(FirecrackerDriver::new());
    let report = block_on_future(driver.probe(ctx.clone()));
    let manager = SandboxManager::new(
        ctx.clone(),
        vec![driver],
        Arc::new(NativeConnector),
        Arc::new(MemoryRecordStore::new()),
        ManagerConfig::default(),
    );
    Ok(ControlRole {
        provider: Arc::new(manager),
        can_control: report.usable,
    })
}
