//! VM profile definitions, guest OS types, and display modes.
//!
//! All profiles ship with sane defaults. Users can override via
//! `~/.config/foundation_testbed/config.toml`.

use serde::{Deserialize, Serialize};

// ── Enums ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuestOs {
    Windows,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Headless,
    Headful,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    Full,
    SshOnly,
}

// ── Profile ──────────────────────────────────────────────────────────────────

/// Configuration for a single VM profile.
///
/// Profiles are defined statically here and can be overridden by user config.
#[derive(Debug, Clone)]
pub struct VmProfile {
    /// Logical name used for state files (e.g. "windows-build").
    pub name: &'static str,
    /// Guest operating system.
    pub os: GuestOs,
    /// Base image filename in the image cache (e.g. "windows-11-x86_64.qcow2").
    pub image_name: &'static str,
    /// Host port forwarded to guest SSH (:22).
    pub ssh_port: u16,
    /// Host port forwarded to guest RDP (:3389), if applicable.
    pub rdp_port: Option<u16>,
    /// Host port forwarded to guest WinRM (:5985), if applicable.
    pub winrm_port: Option<u16>,
    /// VNC display port (VNC server runs on 5900 + offset).
    pub vnc_port: u16,
    /// Default guest username.
    pub user: &'static str,
    /// Default guest password.
    pub pass: &'static str,
    /// Bootstrap scope.
    pub bootstrap: BootstrapMode,
    /// Guest RAM in MiB.
    pub memory_mib: u32,
    /// Guest CPU cores.
    pub cpu_cores: u32,
    /// Guest disk size in GB (used when creating a new qcow2).
    pub disk_gb: u32,
    /// Optional direct-download URL for a pre-baked qcow2 image.
    pub prebaked_url: Option<&'static str>,
}

impl VmProfile {
    /// Resolve the cache path for this profile's disk image.
    pub fn image_cache_path(&self) -> std::path::PathBuf {
        image_cache_dir().join(self.image_name)
    }
}

// ── Default Profiles ─────────────────────────────────────────────────────────

const PROFILES: &[VmProfile] = &[
    VmProfile {
        name: "windows-build",
        os: GuestOs::Windows,
        image_name: "windows-11-x86_64.qcow2",
        ssh_port: 2222,
        rdp_port: Some(3389),
        winrm_port: Some(5985),
        vnc_port: 5900,
        user: "vagrant",
        pass: "vagrant",
        bootstrap: BootstrapMode::Full,
        memory_mib: 12_288,
        cpu_cores: 4,
        disk_gb: 80,
        prebaked_url: None,
    },
    VmProfile {
        name: "windows-test",
        os: GuestOs::Windows,
        image_name: "windows-11-x86_64.qcow2",
        ssh_port: 2322,
        rdp_port: Some(3389),
        winrm_port: Some(5985),
        vnc_port: 5901,
        user: "vagrant",
        pass: "vagrant",
        bootstrap: BootstrapMode::SshOnly,
        memory_mib: 4096,
        cpu_cores: 2,
        disk_gb: 40,
        prebaked_url: None,
    },
    VmProfile {
        name: "linux-build",
        os: GuestOs::Linux,
        image_name: "ubuntu-24.04-x86_64.qcow2",
        ssh_port: 2422,
        rdp_port: None,
        winrm_port: None,
        vnc_port: 5902,
        user: "vagrant",
        pass: "vagrant",
        bootstrap: BootstrapMode::Full,
        memory_mib: 4096,
        cpu_cores: 4,
        disk_gb: 40,
        prebaked_url: None,
    },
    VmProfile {
        name: "linux-test",
        os: GuestOs::Linux,
        image_name: "ubuntu-24.04-x86_64.qcow2",
        ssh_port: 2522,
        rdp_port: None,
        winrm_port: None,
        vnc_port: 5903,
        user: "vagrant",
        pass: "vagrant",
        bootstrap: BootstrapMode::SshOnly,
        memory_mib: 2048,
        cpu_cores: 2,
        disk_gb: 20,
        prebaked_url: None,
    },
];

/// Look up a profile by name.
pub fn get_profile(name: &str) -> Result<&'static VmProfile> {
    PROFILES
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            let available: Vec<&str> = PROFILES.iter().map(|p| p.name).collect();
            TestbedError::UnknownProfile {
                name: name.to_string(),
                available: available.iter().map(|s| s.to_string()).collect(),
            }
        })
}

/// List all default profiles.
pub fn list_profiles() -> &'static [VmProfile] {
    PROFILES
}

// ── Directories ──────────────────────────────────────────────────────────────

/// Base cache directory: `~/.cache/foundation_testbed/`
pub fn cache_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("foundation_testbed")
}

/// Image cache: `~/.cache/foundation_testbed/images/`
pub fn image_cache_dir() -> std::path::PathBuf {
    cache_dir().join("images")
}

/// State directory: `~/.cache/foundation_testbed/state/`
pub fn state_dir() -> std::path::PathBuf {
    cache_dir().join("state")
}

/// Monitor socket directory: `/tmp/foundation_testbed/`
pub fn monitor_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("/tmp").join("foundation_testbed")
}

/// Ensure all foundation_testbed cache directories exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir())?;
    std::fs::create_dir_all(image_cache_dir())?;
    std::fs::create_dir_all(state_dir())?;
    std::fs::create_dir_all(monitor_dir())?;
    Ok(())
}

// ── Error types ──────────────────────────────────────────────────────────────

use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum TestbedError {
    #[display("unknown VM profile '{name}'; available: {}", available.join(", "))]
    UnknownProfile { name: String, available: Vec<String> },

    #[display("QEMU not found on PATH — install via: {install_cmd}")]
    QemuNotFound { install_cmd: String },

    #[display("KVM not available — ensure kvm_intel/kvm_amd module is loaded")]
    KvmUnavailable,

    #[display("VM '{name}' is not running")]
    VmNotRunning { name: String },

    #[display("SSH connection failed on port {port}: {source}")]
    SshFailed { port: u16, source: anyhow::Error },

    #[display("WinRM not reachable on port {port}")]
    WinrmNotReachable { port: u16 },

    #[display("Build failed for target '{target}' (exit {code})")]
    BuildFailed { target: String, code: i32 },

    #[display("VM image download failed: HTTP {status} from {url}")]
    DownloadFailed { status: u16, url: String },

    #[display("Bootstrap failed at step '{step}': {message}")]
    BootstrapFailed { step: String, message: String },

    #[display("Artifact not found at '{path}' on VM")]
    ArtifactNotFound { path: String },

    #[display("Disk resize failed: {source}")]
    DiskResizeFailed { source: anyhow::Error },

    #[display("Port {port} is already in use")]
    PortInUse { port: u16 },

    #[display("Snapshot '{name}' failed: {reason}")]
    SnapshotFailed { name: String, reason: String },

    #[display("QEMU process exited unexpectedly (exit {code})")]
    QemuExited { code: i32 },

    #[display("QEMU monitor communication failed: {source}")]
    MonitorFailed { source: anyhow::Error },

    #[display("Failed to locate qemu-img — install via: {install_cmd}")]
    QemuImgNotFound { install_cmd: String },

    #[display("qcow2 operation failed: {message}")]
    Qcow2Error { message: String },
}

pub type Result<T> = std::result::Result<T, TestbedError>;
