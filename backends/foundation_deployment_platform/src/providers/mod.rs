//! Provider trait — hypervisor backend abstraction.
//!
//! All VM operations go through this trait, enabling different
//! backends (QEMU on Linux, UTM on macOS) to be swapped transparently.

#[cfg(feature = "vms")]
use std::path::PathBuf;
#[cfg(feature = "vms")]
use crate::config::{DisplayMode, Result, VmProfile};
#[cfg(feature = "vms")]
use crate::doctor::HostHealth;

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
    pub internal_id: String, // PID for QEMU, UUID for UTM
    pub resolved_ports: ResolvedPorts,
    pub display_mode: DisplayMode,
}

impl VmHandle {
    /// Check if this handle is for a QEMU provider.
    pub fn is_qemu(&self) -> bool {
        self.provider_id == ProviderId::Qemu
    }

    /// Get the QEMU process PID, if applicable.
    pub fn pid(&self) -> Option<i32> {
        if self.is_qemu() {
            self.internal_id.parse::<i32>().ok()
        } else {
            None
        }
    }
}

/// A VM backend provider (QEMU, UTM). Uses concrete types — no associated types.
///
/// TODO(Feature 10): Migrate to the associated-types `Provider` trait alongside
/// Docker. `DisplayMode` will move to provider construction; `monitor_command`
/// and `ensure_image` will become inherent methods.
pub trait VmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle>;
    fn stop(&self, handle: &VmHandle) -> Result<()>;
    fn is_running(&self, handle: &VmHandle) -> bool;
    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts>;
    fn monitor_command(&self, handle: &VmHandle, cmd: &str) -> Result<String>;
    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf>;
    fn host_health(&self) -> HostHealth;
}

/// Select the appropriate provider based on the host OS.
#[cfg(target_os = "linux")]
pub fn default_provider() -> Result<Box<dyn VmProvider>> {
    Ok(Box::new(qemu::QemuProvider::new()))
}

#[cfg(target_os = "macos")]
pub fn default_provider() -> Result<Box<dyn VmProvider>> {
    Ok(Box::new(utm::UtmProvider::new()))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn default_provider() -> Result<Box<dyn VmProvider>> {
    Err(crate::config::TestbedError::Qcow2Error {
        message: "testbed only supports Linux (QEMU) and macOS (UTM)".to_string(),
    })
}

// ── Associated-types Provider trait (Decision 03, Feature 10) ─────────────────

/// A backend provider with associated types for handle, config, and error.
///
/// DockerProvider implements this directly. QEMU/UTM will migrate from
/// [`VmProvider`] to this trait (Feature 10).
pub trait Provider: Send + Sync {
    type Handle;
    type Config;
    type Error: std::fmt::Debug + std::fmt::Display;

    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    fn launch(&self, config: &Self::Config) -> std::result::Result<Self::Handle, Self::Error>;
    fn stop(&self, handle: &Self::Handle) -> std::result::Result<(), Self::Error>;
    fn is_running(&self, handle: &Self::Handle) -> bool;
    fn resolved_ports(&self, handle: &Self::Handle) -> std::result::Result<ResolvedPorts, Self::Error>;
    fn host_health(&self) -> HostHealth;
}

// ── Provider backend modules ─────────────────────────────────────────────────

pub mod docker;
pub mod http;
pub mod qemu;
pub mod utm;

