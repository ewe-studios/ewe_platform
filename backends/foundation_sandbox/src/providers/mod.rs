//! The `Provider` facade over any control plane (feature 06).
//!
//! The `Provider` trait itself stays in `foundation_deployment_platform`; this
//! module adapts one control plane to it (decision D1). A method here only
//! translates: it blocks on the matching `SandboxProvider` call and returns its
//! outcome (invariant I7). Because the facade takes any `SandboxProvider`, the
//! same type works on a local `SandboxManager` and on a `SandboxRpcClient`.
//!
//! Every method that blocks calls
//! [`block_on_future`](foundation_core::valtron::block_on_future), so the caller
//! must start the valtron pool first. Async callers use `SandboxProvider`
//! directly.

use std::ops::Deref;
use std::sync::Arc;

use foundation_core::valtron::block_on_future;
use foundation_deployment_platform::providers::{Provider, ProviderId, ResolvedPorts};

use crate::shared::control::{SandboxHandle, SandboxProvider, SandboxState};
use crate::shared::error::SandboxResult;
use crate::shared::ids::DriverId;
use crate::shared::spec::SandboxSpec;

/// A running sandbox, as a [`Provider`] handle.
#[derive(Clone)]
pub struct SandboxDeploymentHandle {
    /// The handle of the running sandbox.
    pub sandbox: SandboxHandle,
    /// The provider that launched it.
    pub provider_id: ProviderId,
}

impl Deref for SandboxDeploymentHandle {
    type Target = SandboxHandle;

    fn deref(&self) -> &Self::Target {
        &self.sandbox
    }
}

/// A [`Provider`] on any control plane.
///
/// `driver` fixes which driver each launch uses. A launch copies the spec and
/// replaces its `driver` with this value.
pub struct SandboxDeploymentProvider {
    control: Arc<dyn SandboxProvider>,
    driver: DriverId,
}

impl SandboxDeploymentProvider {
    /// A provider that launches Firecracker sandboxes.
    #[must_use]
    pub fn firecracker(control: Arc<dyn SandboxProvider>) -> Self {
        Self::new(control, DriverId::Firecracker)
    }

    /// A provider that launches Cloudflare container sandboxes.
    #[must_use]
    pub fn cloudflare(control: Arc<dyn SandboxProvider>) -> Self {
        Self::new(control, DriverId::CloudflareContainer)
    }

    /// A provider that launches libkrun sandboxes.
    #[must_use]
    pub fn libkrun(control: Arc<dyn SandboxProvider>) -> Self {
        Self::new(control, DriverId::Libkrun)
    }

    /// A provider that launches sandboxes with `driver`.
    #[must_use]
    pub fn new(control: Arc<dyn SandboxProvider>, driver: DriverId) -> Self {
        Self { control, driver }
    }
}

impl Provider for SandboxDeploymentProvider {
    type Handle = SandboxDeploymentHandle;
    type Config = SandboxSpec;
    type Error = foundation_errstacks::ErrorTrace<crate::shared::error::SandboxError>;

    fn name(&self) -> &'static str {
        driver_name(self.driver)
    }

    fn id(&self) -> ProviderId {
        provider_id(self.driver)
    }

    fn launch(&self, config: &Self::Config) -> SandboxResult<Self::Handle> {
        let control = Arc::clone(&self.control);
        let mut spec = config.clone();
        spec.driver = Some(self.driver);
        let sandbox = block_on_future(async move { control.create(spec).await })?;
        Ok(SandboxDeploymentHandle {
            sandbox,
            provider_id: provider_id(self.driver),
        })
    }

    fn stop(&self, handle: &Self::Handle) -> SandboxResult<()> {
        let sandbox = handle.sandbox.clone();
        block_on_future(async move { sandbox.stop().await }).map(|_report| ())
    }

    fn is_running(&self, handle: &Self::Handle) -> bool {
        handle.sandbox.info().state == SandboxState::Ready
    }

    fn resolved_ports(&self, _handle: &Self::Handle) -> SandboxResult<ResolvedPorts> {
        // A sandbox has no SSH, WinRM, RDP or VNC port. `DockerProvider` also
        // returns 0 for a port that the container does not publish.
        Ok(ResolvedPorts {
            ssh_port: 0,
            winrm_port: None,
            rdp_port: None,
            vnc_port: 0,
        })
    }

    fn host_health(&self) -> Vec<(String, bool, String)> {
        let control = Arc::clone(&self.control);
        let driver = self.driver;
        match block_on_future(async move { control.probe().await }) {
            Ok(reports) => reports
                .into_iter()
                .find(|report| report.driver == driver)
                .map_or_else(Vec::new, |report| {
                    report
                        .checks
                        .into_iter()
                        .map(|check| (check.name, check.ok, check.message))
                        .collect()
                }),
            Err(error) => vec![(
                "probe".to_string(),
                false,
                error.current_context().to_string(),
            )],
        }
    }
}

/// The provider of one driver.
///
/// A libkrun sandbox has its own provider identifier. A driver without one
/// keeps the Firecracker identifier: the caller reads the driver from the
/// sandbox info. A Cloudflare Durable Object sandbox is not a container, so it
/// keeps the Cloudflare provider identifier too.
fn provider_id(driver: DriverId) -> ProviderId {
    match driver {
        DriverId::Libkrun => ProviderId::Libkrun,
        DriverId::CloudflareContainer | DriverId::CloudflareDo => ProviderId::CloudflareContainer,
        DriverId::Firecracker | DriverId::Docker => ProviderId::Firecracker,
    }
}

/// The static name of one driver, for [`Provider::name`].
fn driver_name(driver: DriverId) -> &'static str {
    match driver {
        DriverId::Firecracker => "firecracker",
        DriverId::Libkrun => "libkrun",
        DriverId::Docker => "docker",
        DriverId::CloudflareContainer => "cloudflare-container",
        DriverId::CloudflareDo => "cloudflare-do",
    }
}
