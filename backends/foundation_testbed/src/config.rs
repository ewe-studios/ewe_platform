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
    MacOS,
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
    VmProfile {
        name: "macos-build",
        os: GuestOs::MacOS,
        image_name: "macos-sonoma-x86_64.qcow2",
        ssh_port: 2622,
        rdp_port: None,
        winrm_port: None,
        vnc_port: 5904,
        user: "vagrant",
        pass: "vagrant",
        bootstrap: BootstrapMode::SshOnly,
        memory_mib: 8192,
        cpu_cores: 4,
        disk_gb: 80,
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

// ── User Config Override ─────────────────────────────────────────────────────

/// User-provided profile overrides from `testbed.toml` (cwd by default).
///
/// Fields are optional — only specified fields override the default profile.
#[derive(Debug, Clone, Deserialize)]
pub struct UserVmProfile {
    pub os: Option<String>,
    pub image_name: Option<String>,
    pub ssh_port: Option<u16>,
    pub rdp_port: Option<u16>,
    pub winrm_port: Option<u16>,
    pub vnc_port: Option<u16>,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub bootstrap: Option<String>,
    pub memory_mib: Option<u32>,
    pub cpu_cores: Option<u32>,
    pub disk_gb: Option<u32>,
    pub prebaked_url: Option<String>,
}

/// Top-level user config file format.
#[derive(Debug, Clone, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub profiles: Vec<UserProfileEntry>,
}

/// A named profile entry in the user config.
#[derive(Debug, Clone, Deserialize)]
pub struct UserProfileEntry {
    pub name: String,
    #[serde(flatten)]
    pub profile: UserVmProfile,
}

/// Load user config from `testbed.toml` in the current working directory.
///
/// Returns None if the file doesn't exist or can't be parsed.
pub fn load_user_config() -> Option<UserConfig> {
    let config_path = user_config_path();
    if !config_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&config_path).ok()?;
    toml::from_str(&content).ok()
}

/// Resolve a profile, applying any user config overrides.
pub fn get_profile_with_config(name: &str) -> Result<VmProfile> {
    let base = get_profile(name)?;
    let mut profile = base.clone();

    // Apply user config override if present
    if let Some(config) = load_user_config()
        && let Some(entry) = config.profiles.iter().find(|e| e.name == name) {
            apply_user_override(&mut profile, &entry.profile);
        }

    Ok(profile)
}

/// Apply user overrides to a profile.
fn apply_user_override(profile: &mut VmProfile, override_: &UserVmProfile) {
    let o = override_;

    if let Some(ref os) = o.os {
        profile.os = match os.as_str() {
            "windows" => GuestOs::Windows,
            "linux" => GuestOs::Linux,
            "macos" => GuestOs::MacOS,
            _ => profile.os,
        };
    }
    if let Some(ref image_name) = o.image_name {
        profile.image_name = Box::leak(image_name.clone().into_boxed_str());
    }
    if let Some(p) = o.ssh_port {
        profile.ssh_port = p;
    }
    if let Some(p) = o.rdp_port {
        profile.rdp_port = Some(p);
    }
    if let Some(p) = o.winrm_port {
        profile.winrm_port = Some(p);
    }
    if let Some(p) = o.vnc_port {
        profile.vnc_port = p;
    }
    if let Some(ref user) = o.user {
        profile.user = Box::leak(user.clone().into_boxed_str());
    }
    if let Some(ref pass) = o.pass {
        profile.pass = Box::leak(pass.clone().into_boxed_str());
    }
    if let Some(ref bootstrap) = o.bootstrap {
        profile.bootstrap = match bootstrap.as_str() {
            "full" => BootstrapMode::Full,
            "ssh_only" => BootstrapMode::SshOnly,
            _ => profile.bootstrap,
        };
    }
    if let Some(m) = o.memory_mib {
        profile.memory_mib = m;
    }
    if let Some(c) = o.cpu_cores {
        profile.cpu_cores = c;
    }
    if let Some(d) = o.disk_gb {
        profile.disk_gb = d;
    }
    if let Some(ref url) = o.prebaked_url {
        profile.prebaked_url = Some(Box::leak(url.clone().into_boxed_str()));
    }
}

/// Path to the user config file: `./testbed.toml`.
pub fn user_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from("testbed.toml")
}

// ── Virtio ISO for Windows guests ────────────────────────────────────────────

/// Local store path for the virtio-win driver ISO (checked first).
pub const VIRTIO_ISO_STORE_PATH: &str = "/home/darkvoid/EweStore/Testbed/virtio-win-0.1.262.iso";

/// Fedora Project download URL for virtio-win drivers (fallback).
pub const VIRTIO_ISO_DOWNLOAD_URL: &str = "https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/archive-virtio/virtio-win-0.1.262-1/virtio-win-0.1.262.iso";

// ── Image Stores ─────────────────────────────────────────────────────────────

/// Type of remote/local storage for VM images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreType {
    R2,
    S3,
    Local,
    Http,
}

/// An image store where exported VMs can be uploaded.
#[derive(Debug, Clone)]
pub struct ImageStore {
    pub name: String,
    pub store_type: StoreType,
    /// Bucket name (for R2/S3) or directory path (for Local) or base URL (for Http).
    pub destination: String,
    /// Remote key prefix (e.g., "images/linux-build/"). Empty for local.
    pub key_prefix: Option<String>,
}

/// Load image stores from `testbed.toml`'s `[[image_stores]]` section.
pub fn load_image_stores() -> Vec<ImageStore> {
    let Some(_config) = load_user_config() else {
        return Vec::new();
    };
    // Image stores would be parsed from a dedicated section; for now,
    // return empty — stores must be defined in testbed.toml.
    Vec::new()
}

/// Look up an image store by name.
pub fn get_image_store(name: &str) -> Option<ImageStore> {
    load_image_stores()
        .into_iter()
        .find(|s| s.name == name)
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

/// VM work directory: `$PWD/.testbed/[vm-name]/` — per-VM logs and artifacts.
pub fn work_dir(profile_name: &str) -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".testbed")
        .join(profile_name)
}

/// Ensure all foundation_testbed cache directories exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir())?;
    std::fs::create_dir_all(image_cache_dir())?;
    std::fs::create_dir_all(state_dir())?;
    std::fs::create_dir_all(monitor_dir())?;
    std::fs::create_dir_all(std::env::current_dir().unwrap_or_default().join(".testbed"))?;
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

    #[display("WinRM operation timed out: {message}")]
    WinrmTimeout { message: String },

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
