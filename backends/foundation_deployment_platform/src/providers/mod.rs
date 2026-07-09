//! Provider trait — hypervisor backend abstraction.
//!
//! **WHY:** All VM/container operations go through this trait, enabling
//! different backends (QEMU on Linux, UTM on macOS, Docker anywhere)
//! to be swapped transparently via the CLI and build pipeline.
//!
//! **WHAT:** The `Provider` trait uses associated types for `Handle`
//! and `Config` — each backend defines its own (QEMU → VmHandle +
//! VmProfile, Docker → ContainerHandle + ContainerServiceDefinition).
//! `PlatformHandle` and `PlatformProfile` enums dispatch to the
//! concrete backend at runtime.
//!
//! **HOW:** `default_provider()` selects based on the host OS and
//! available runtimes (Docker daemon, KVM, UTM). Callers use the
//! enum dispatch to work with any backend without generic bounds.

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
            Self::Qemu => write!(f, "qemu"),
            Self::Utm => write!(f, "utm"),
            Self::Docker => write!(f, "docker"),
        }
    }
}

/// Resolved network ports for a running VM or container.
#[derive(Debug, Clone)]
pub struct ResolvedPorts {
    pub ssh_port: u16,
    pub winrm_port: Option<u16>,
    pub rdp_port: Option<u16>,
    pub vnc_port: u16,
}

/// Host health check result.
#[derive(Debug, Clone)]
pub struct HostHealth {
    pub provider: String,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub passed: bool,
    pub detail: Option<String>,
}

impl HostHealth {
    pub fn new(provider: &str) -> Self {
        Self { provider: provider.to_string(), checks: Vec::new() }
    }

    pub fn check(&mut self, name: &str, f: impl FnOnce() -> bool) {
        let passed = f();
        self.checks.push(HealthCheck { name: name.to_string(), passed, detail: None });
    }
}

/// A hypervisor backend provider.
pub trait Provider: Send + Sync {
    /// The handle returned by `launch()`.
    type Handle;
    /// The configuration accepted by `launch()`.
    type Config;
    /// The error type for fallible operations.
    type Error: std::fmt::Debug + std::fmt::Display;

    fn name(&self) -> &'static str;
    fn id(&self) -> ProviderId;
    fn launch(&self, config: &Self::Config) -> Result<Self::Handle, Self::Error>;
    fn stop(&self, handle: &Self::Handle) -> Result<(), Self::Error>;
    fn is_running(&self, handle: &Self::Handle) -> bool;
    fn resolved_ports(&self, handle: &Self::Handle) -> Result<ResolvedPorts, Self::Error>;
    fn host_health(&self) -> HostHealth;
}

pub mod docker;
