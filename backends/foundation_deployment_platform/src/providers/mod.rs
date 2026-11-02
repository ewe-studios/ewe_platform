//! Provider trait — hypervisor backend abstraction.
//!
//! All VM operations go through this trait, enabling different
//! backends (QEMU on Linux, UTM on macOS) to be swapped transparently.

#[cfg(feature = "vms")]
use crate::config::{DisplayMode, Result, VmProfile};
#[cfg(feature = "vms")]
use crate::doctor::HostHealth;
#[cfg(feature = "vms")]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Unique identifier for a provider backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderId {
    Qemu,
    Utm,
    Docker,
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderId::Qemu => write!(f, "qemu"),
            ProviderId::Utm => write!(f, "utm"),
            ProviderId::Docker => write!(f, "docker"),
        }
    }
}

/// Resolved network ports for a running VM.
#[derive(Debug, Clone)]
pub struct ResolvedPorts {
    pub ssh_port: u16,
    pub winrm_port: Option<u16>,
    pub rdp_port: Option<u16>,
    pub vnc_port: u16,
}

/// Handle to a running VM, returned by the provider after launch.
#[cfg(feature = "vms")]
pub struct VmHandle {
    pub profile: VmProfile,
    pub provider_id: ProviderId,
    pub internal_id: String,
    pub resolved_ports: ResolvedPorts,
    pub display_mode: DisplayMode,
}

#[cfg(feature = "vms")]
impl VmHandle {
    #[must_use]
    pub fn is_qemu(&self) -> bool {
        self.provider_id == ProviderId::Qemu
    }
    #[must_use]
    pub fn pid(&self) -> Option<i32> {
        if self.is_qemu() {
            self.internal_id.parse::<i32>().ok()
        } else {
            None
        }
    }
}

#[cfg(feature = "vms")]
pub trait VmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    /// Start a VM from a profile.
    ///
    /// # Errors
    /// Returns an error when the backend cannot start the VM.
    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle>;
    /// Stop a running VM.
    ///
    /// # Errors
    /// Returns an error when the backend cannot stop the VM.
    fn stop(&self, handle: &VmHandle) -> Result<()>;
    fn is_running(&self, handle: &VmHandle) -> bool;
    /// Resolve the network ports of a running VM.
    ///
    /// # Errors
    /// Returns an error when the backend cannot resolve the ports.
    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts>;
    /// Send a command to the VM monitor.
    ///
    /// # Errors
    /// Returns an error when the backend has no monitor.
    fn monitor_command(&self, handle: &VmHandle, cmd: &str) -> Result<String>;
    /// Ensure the VM disk image exists.
    ///
    /// # Errors
    /// Returns an error when the image cannot be prepared.
    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf>;
    fn host_health(&self) -> HostHealth;
}

#[cfg(feature = "vms")]
#[cfg(target_os = "linux")]
/// Return the default provider for the current platform.
///
/// # Errors
/// Returns an error if the platform has no supported provider.
pub fn default_provider() -> Result<Box<dyn VmProvider>> {
    Ok(Box::new(qemu::QemuProvider::new()))
}

#[cfg(feature = "vms")]
#[cfg(target_os = "macos")]
/// Return the default provider for the current platform.
///
/// # Errors
/// Returns an error if the platform has no supported provider.
pub fn default_provider() -> Result<Box<dyn VmProvider>> {
    Ok(Box::new(utm::UtmProvider::new()))
}

#[cfg(feature = "vms")]
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
/// Return the default provider for the current platform.
///
/// # Errors
/// Returns an error if the platform has no supported provider.
pub fn default_provider() -> Result<Box<dyn VmProvider>> {
    Err(crate::config::TestbedError::Qcow2Error {
        message: "testbed only supports Linux (QEMU) and macOS (UTM)".to_string(),
    })
}

// ── Associated-types Provider trait (Decision 03, Feature 10) ─────────────────

/// A backend provider with associated types for handle, config, and error.
///
/// `DockerProvider` implements this directly. QEMU/UTM will migrate from
/// [`VmProvider`] to this trait (Feature 10).
pub trait Provider: Send + Sync {
    type Handle;
    type Config;
    type Error: std::fmt::Debug + std::fmt::Display;

    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    /// # Errors
    /// Returns an error when the backend cannot start the VM or container.
    fn launch(&self, config: &Self::Config) -> std::result::Result<Self::Handle, Self::Error>;
    /// # Errors
    /// Returns an error when the backend cannot stop the VM or container.
    fn stop(&self, handle: &Self::Handle) -> std::result::Result<(), Self::Error>;
    fn is_running(&self, handle: &Self::Handle) -> bool;
    /// # Errors
    /// Returns an error when the backend cannot resolve the running ports.
    fn resolved_ports(
        &self,
        handle: &Self::Handle,
    ) -> std::result::Result<ResolvedPorts, Self::Error>;
    fn host_health(&self) -> Vec<(String, bool, String)>;
}

// ── Provider backend modules ─────────────────────────────────────────────────

pub mod docker;
#[cfg(feature = "vms")]
pub mod http;
#[cfg(feature = "vms")]
pub mod qemu;
#[cfg(feature = "vms")]
pub mod utm;
