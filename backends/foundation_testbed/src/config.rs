//! VM profile definitions, guest OS types, and display modes.
//!
//! All profiles ship with sane defaults. Users can override via
//! `~/.config/foundation_testbed/config.toml`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

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
    None,
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
        name: "windows-11-bootstrapped",
        os: GuestOs::Windows,
        image_name: "windows-11-x86_64-bootstrapped.qcow2",
        ssh_port: 2422,
        rdp_port: Some(3389),
        winrm_port: Some(5985),
        vnc_port: 5902,
        user: "vagrant",
        pass: "vagrant",
        bootstrap: BootstrapMode::None,
        memory_mib: 12_288,
        cpu_cores: 4,
        disk_gb: 80,
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
    /// Per-VM mount configuration.
    #[serde(default)]
    pub mount: Option<UserMountConfig>,
    /// Per-VM artifact overrides: name → host path.
    /// e.g. `artefacts = { "vsb_layout.windows.zip" = "/path/to/zip" }`
    #[serde(default)]
    pub artefacts: HashMap<String, String>,
    /// Per-VM state directory for sockets and ephemeral state.
    /// Defaults to `$PWD/.testbed/[vm-name]/state/` if not specified.
    #[serde(default)]
    pub state_directory: Option<String>,
}

/// Per-VM mount configuration from testbed.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct UserMountConfig {
    /// Host path to mount (default: ".")
    #[serde(default)]
    pub host_path: Option<String>,
    /// Guest mount point (e.g. "C:/Users/vagrant/project" on Windows, "/mnt/project" on Linux)
    #[serde(default)]
    pub guest_path: Option<String>,
    #[serde(default)]
    pub readonly: Option<bool>,
    /// Mount methods to try, in order. Default: ["smb", "virtiofs"] for Windows, ["9p"] for Linux.
    #[serde(default)]
    pub methods: Option<Vec<String>>,
}

/// Mount method enum for Windows guests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountMethod {
    SMB,
    Virtiofs,
}

impl MountMethod {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "smb" => Some(MountMethod::SMB),
            "virtiofs" | "virtio-fs" => Some(MountMethod::Virtiofs),
            _ => None,
        }
    }
}

/// Deserialized `[[image_stores]]` entry from testbed.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageStoreEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub store_type: String,
    pub destination: String,
    #[serde(default)]
    pub key_prefix: Option<String>,
    #[serde(default)]
    pub registry: Option<String>,
}

/// Top-level user config file format.
#[derive(Debug, Clone, Deserialize)]
pub struct UserConfig {
    #[serde(default, rename = "vms", alias = "profiles")]
    pub profiles: Vec<UserProfileEntry>,
    #[serde(default)]
    pub image_stores: Vec<ImageStoreEntry>,
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
            "none" => BootstrapMode::None,
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

/// Get the mount configuration for a VM profile, applying user overrides.
///
/// Returns `Some((host_path, guest_path, readonly, methods))` if mount is configured,
/// or `None` if no mount should be set up.
/// `methods` is a list of mount methods to try in order (e.g. `["smb", "virtiofs"]`).
pub fn get_mount_for_profile(
    profile_name: &str,
    default_host_path: &str,
) -> Option<(String, String, bool, Vec<String>)> {
    let os = get_profile(profile_name).ok()?.os;
    let default_guest = match os {
        GuestOs::Windows => "C:/Users/vagrant/project",
        GuestOs::Linux => "/mnt/project",
        GuestOs::MacOS => "/Users/vagrant/project",
    };
    let default_methods = match os {
        GuestOs::Windows => vec!["smb".to_string(), "virtiofs".to_string()],
        GuestOs::Linux | GuestOs::MacOS => vec!["9p".to_string()],
    };

    if let Some(config) = load_user_config()
        && let Some(entry) = config.profiles.iter().find(|e| e.name == profile_name)
        && let Some(mount) = &entry.profile.mount {
            let host_path = mount.host_path.as_deref().unwrap_or(default_host_path).to_string();
            let guest_path = mount.guest_path.as_deref().unwrap_or(default_guest).to_string();
            let readonly = mount.readonly.unwrap_or(false);
            let methods = mount.methods.clone().unwrap_or(default_methods);
            return Some((host_path, guest_path, readonly, methods));
        }

    // Fallback: use defaults
    Some((default_host_path.to_string(), default_guest.to_string(), false, default_methods))
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
    let Some(config) = load_user_config() else {
        return Vec::new();
    };
    config
        .image_stores
        .into_iter()
        .filter_map(|e| {
            let store_type = match e.store_type.as_str() {
                "r2" => StoreType::R2,
                "s3" => StoreType::S3,
                "local" => StoreType::Local,
                "http" | "https" => StoreType::Http,
                "vagrant" => StoreType::Http, // treat as http for now
                _ => return None,
            };
            Some(ImageStore {
                name: e.name,
                store_type,
                destination: e.destination,
                key_prefix: e.key_prefix,
            })
        })
        .collect()
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

/// Monitor socket directory for a VM: `$PWD/.testbed/[vm-name]/state/` or from `testbed.toml` `[[vms]].state_directory`.
/// Per-VM sockets and ephemeral state.
pub fn monitor_dir(profile_name: &str) -> std::path::PathBuf {
    // Check if user config specifies a custom state directory for this VM
    if let Some(config) = load_user_config() {
        if let Some(entry) = config.profiles.iter().find(|e| e.name == profile_name) {
            if let Some(ref custom_dir) = entry.profile.state_directory {
                return std::path::PathBuf::from(custom_dir);
            }
        }
    }
    // Default: $PWD/.testbed/[vm-name]/state/
    work_dir(profile_name).join("state")
}

/// Find workspace root by searching for the directory containing Cargo.toml with [workspace]
pub fn workspace_root() -> std::path::PathBuf {
    let current = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut path = current.clone();

    loop {
        let cargo_toml = path.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return path;
                }
            }
        }
        if !path.pop() {
            break;
        }
    }

    // Fallback: check for .testbed directory in parent
    path = current.clone();
    loop {
        if path.join(".testbed").exists() {
            return path;
        }
        if !path.pop() {
            break;
        }
    }

    current
}

/// VM work directory: `$WORKSPACE_ROOT/.testbed/[vm-name]/` — per-VM logs and artifacts.
pub fn work_dir(profile_name: &str) -> std::path::PathBuf {
    workspace_root().join(".testbed").join(profile_name)
}

/// Per-VM logs directory: `$PWD/.testbed/[vm-name]/logs/`.
pub fn vm_logs_dir(profile_name: &str) -> std::path::PathBuf {
    work_dir(profile_name).join("logs")
}

/// Per-VM artifacts directory: `$PWD/.testbed/[vm-name]/artifacts/`.
pub fn vm_artifacts_dir(profile_name: &str) -> std::path::PathBuf {
    work_dir(profile_name).join("artifacts")
}

/// Home-based artifacts directory: `$HOME/.testbed/[vm-name]/artifacts/`.
pub fn home_artifacts_dir(profile_name: &str) -> std::path::PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".testbed").join(profile_name).join("artifacts"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/.testbed").join(profile_name).join("artifacts"))
}

/// Resolve an artifact by checking in order:
/// 1. Explicit path in `testbed.toml` `[[vms]]` artefacts map
/// 2. Project-local artifact dir `$PWD/.testbed/[vm]/artifacts/`
/// 3. Home artifact dir `$HOME/.testbed/[vm]/artifacts/`
///
/// Returns the first matching path, or `None` if not found.
pub fn resolve_artifact(profile_name: &str, filename: &str) -> Option<std::path::PathBuf> {
    // 1. Check config-defined override
    if let Some(config) = load_user_config() {
        if let Some(entry) = config.profiles.iter().find(|e| e.name == profile_name) {
            if let Some(path) = entry.profile.artefacts.get(filename) {
                let p = std::path::PathBuf::from(path);
                if p.exists() {
                    debug!("Using config-defined artifact: {} → {}", filename, p.display());
                    return Some(p);
                }
                debug!("Config-defined artifact {} at {} does not exist", filename, p.display());
            }
        }
    }

    // 2. Project-local artifact dir
    let local_dir = vm_artifacts_dir(profile_name);
    let local_path = local_dir.join(filename);
    if local_path.exists() {
        debug!("Using project-local artifact: {}", local_path.display());
        return Some(local_path);
    }

    // 3. Home artifact dir
    let home_dir = home_artifacts_dir(profile_name);
    let home_path = home_dir.join(filename);
    if home_path.exists() {
        debug!("Using home artifact: {}", home_path.display());
        return Some(home_path);
    }

    None
}

/// Resolve a script by checking `$PWD/.testbed/[vm]/scripts/` first, then falling
/// back to the embedded content from the source tree.
///
/// Returns the path to a file with the script content and whether it is a
/// generated temp file. The caller can drop the path without worrying about
/// cleanup — temp files are written to the state directory and overwritten
/// on next run.
pub fn resolve_script_or_embed(
    profile_name: &str,
    filename: &str,
    embedded_content: &str,
) -> Result<std::path::PathBuf> {
    // Check project-local scripts directory first
    let local_path = vm_scripts_dir(profile_name).join(filename);
    if local_path.exists() {
        debug!("Using local script: {}", local_path.display());
        return Ok(local_path);
    }

    // Fall back to embedded content — write to state dir
    let temp_dir = crate::config::state_dir();
    std::fs::create_dir_all(&temp_dir).ok();
    let temp_path = temp_dir.join(filename);
    std::fs::write(&temp_path, embedded_content).map_err(|e| TestbedError::BootstrapFailed {
        step: format!("write embedded script {filename}"),
        message: e.to_string(),
    })?;
    debug!("Using embedded script (no local {} found)", local_path.display());
    Ok(temp_path)
}

/// Copy embedded scripts from the source tree into `$PWD/.testbed/[vm]/scripts/`.
///
/// Called during `ensure_dirs()` or bootstrap init so users can edit scripts
/// in the project-local directory without touching the source tree.
pub fn copy_embedded_scripts(profile_name: &str) -> Result<()> {
    let dest = vm_scripts_dir(profile_name);
    std::fs::create_dir_all(&dest).map_err(|e| TestbedError::BootstrapFailed {
        step: format!("create scripts dir {dest:?}"),
        message: e.to_string(),
    })?;

    // Scripts from the source tree
    let scripts: [(&str, &[u8]); 2] = [
        ("vs_install_from_zip.ps1", include_bytes!("bootstrap/scripts/vs_install_from_zip.ps1").as_slice()),
        ("vs_install_online.ps1", include_bytes!("bootstrap/scripts/vs_install_online.ps1").as_slice()),
    ];

    for (name, content) in &scripts {
        let dest_path = dest.join(name);
        if !dest_path.exists() {
            std::fs::write(&dest_path, content).map_err(|e| TestbedError::BootstrapFailed {
                step: format!("copy script {name}"),
                message: e.to_string(),
            })?;
        }
    }
    Ok(())
}

/// Per-VM scripts directory: `$PWD/.testbed/[vm-name]/scripts/`.
pub fn vm_scripts_dir(profile_name: &str) -> std::path::PathBuf {
    work_dir(profile_name).join("scripts")
}

/// Per-VM state directory: `$PWD/.testbed/[vm-name]/state/`.
pub fn vm_state_dir(profile_name: &str) -> std::path::PathBuf {
    work_dir(profile_name).join("state")
}

/// Ensure all foundation_testbed cache directories exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir())?;
    std::fs::create_dir_all(image_cache_dir())?;
    std::fs::create_dir_all(state_dir())?;
    let testbed = std::env::current_dir().unwrap_or_default().join(".testbed");
    std::fs::create_dir_all(&testbed)?;
    std::fs::create_dir_all(testbed.join("artifacts"))?;
    std::fs::create_dir_all(testbed.join("scripts"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mount_from_inline_toml() {
        // Simulate a testbed.toml with inline mount config
        let toml_content = r#"
            [[vms]]
            name = "linux-build"
            profile = "linux-build"
            mount = { host_path = ".", guest_path = "/mnt/project", readonly = true }
        "#;
        let config: UserConfig = toml::from_str(toml_content).expect("failed to parse");
        assert_eq!(config.profiles.len(), 1);
        let entry = &config.profiles[0];
        assert_eq!(entry.name, "linux-build");
        let mount = entry.profile.mount.as_ref().expect("mount should exist");
        assert_eq!(mount.host_path.as_deref(), Some("."));
        assert_eq!(mount.guest_path.as_deref(), Some("/mnt/project"));
        assert_eq!(mount.readonly, Some(true));
    }

    #[test]
    fn test_parse_mount_with_missing_fields() {
        let toml_content = r#"
            [[vms]]
            name = "windows-build"
            profile = "windows-build"
            mount = { host_path = "." }
        "#;
        let config: UserConfig = toml::from_str(toml_content).expect("failed to parse");
        let entry = &config.profiles[0];
        let mount = entry.profile.mount.as_ref().expect("mount should exist");
        assert_eq!(mount.host_path.as_deref(), Some("."));
        assert!(mount.guest_path.is_none());
        assert!(mount.readonly.is_none());
    }

    #[test]
    fn test_parse_mount_with_methods() {
        let toml_content = r#"
            [[vms]]
            name = "windows-build"
            profile = "windows-build"
            mount = { host_path = ".", methods = ["virtiofs", "smb"] }
        "#;
        let config: UserConfig = toml::from_str(toml_content).expect("failed to parse");
        let entry = &config.profiles[0];
        let mount = entry.profile.mount.as_ref().expect("mount should exist");
        assert_eq!(mount.methods, Some(vec!["virtiofs".to_string(), "smb".to_string()]));
    }
}
