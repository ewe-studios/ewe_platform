//! Provider trait — hypervisor backend abstraction.
//!
//! All VM operations go through this trait, enabling different
//! backends (QEMU on Linux, UTM on macOS) to be swapped transparently.

use std::path::PathBuf;

use crate::vms::config::{DisplayMode, Result, VmProfile};
use crate::vms::doctor::HostHealth;

use serde::{Deserialize, Serialize};

/// Unique identifier for a provider backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderId {
    Qemu,
    Utm,
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderId::Qemu => write!(f, "qemu"),
            ProviderId::Utm => write!(f, "utm"),
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

/// A hypervisor backend provider.
///
/// All VM lifecycle operations go through this trait, enabling
/// different backends (QEMU on Linux, UTM on macOS) to be used
/// transparently by the CLI and build pipeline.
pub trait Provider: Send + Sync {
    /// Human-readable provider name ("qemu" or "utm").
    fn name(&self) -> &'static str;

    /// Provider identifier.
    fn id(&self) -> ProviderId;

    /// Launch a VM with the given profile and display mode.
    /// Returns a handle for process management.
    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle>;

    /// Stop a running VM.
    fn stop(&self, handle: &VmHandle) -> Result<()>;

    /// Check if a VM is currently running.
    fn is_running(&self, handle: &VmHandle) -> bool;

    /// Get the resolved network ports for a running VM.
    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts>;

    /// Send a monitor/control command to the VM.
    fn monitor_command(&self, handle: &VmHandle, cmd: &str) -> Result<String>;

    /// Ensure the VM image is available in the cache.
    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf>;

    /// Check host health (binary on PATH, KVM available, etc.).
    fn host_health(&self) -> HostHealth;
}

/// Select the appropriate provider based on the host OS.
#[cfg(target_os = "linux")]
pub fn default_provider() -> Result<Box<dyn Provider>> {
    Ok(Box::new(qemu::QemuProvider::new()))
}

#[cfg(target_os = "macos")]
pub fn default_provider() -> Result<Box<dyn Provider>> {
    Ok(Box::new(utm::UtmProvider::new()))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn default_provider() -> Result<Box<dyn Provider>> {
    Err(crate::vms::config::TestbedError::Qcow2Error {
        message: "testbed only supports Linux (QEMU) and macOS (UTM)".to_string(),
    })
}

// ── Provider backend modules ─────────────────────────────────────────────────

pub mod http;
pub mod qemu;
pub mod utm;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_id_display() {
        assert_eq!(ProviderId::Qemu.to_string(), "qemu");
        assert_eq!(ProviderId::Utm.to_string(), "utm");
    }

    #[test]
    fn test_vm_handle_pid_parsing() {
        let handle = VmHandle {
            profile: crate::vms::config::get_profile("linux-build").unwrap().clone(),
            provider_id: ProviderId::Qemu,
            internal_id: "12345".to_string(),
            resolved_ports: ResolvedPorts {
                ssh_port: 2222,
                winrm_port: Some(5985),
                rdp_port: Some(3389),
                vnc_port: 5900,
            },
            display_mode: DisplayMode::Headless,
        };
        assert!(handle.is_qemu());
        assert_eq!(handle.pid(), Some(12345));
    }

    #[cfg(feature = "qemu")]
    #[test]
    fn test_qemu_provider_creation() {
        let provider = qemu::QemuProvider::new();
        assert_eq!(provider.name(), "qemu");
        assert_eq!(provider.id(), ProviderId::Qemu);
    }

    #[cfg(feature = "qemu")]
    #[test]
    fn test_qemu_provider_host_health() {
        use crate::vms::providers::Provider;
        let provider = qemu::QemuProvider::new();
        let health = provider.host_health();
        // Should return host health struct (checks may fail if KVM not available)
        let _ = health.is_healthy();
    }
}
