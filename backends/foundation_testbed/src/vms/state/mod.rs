//! Persistent VM state management.
//!
//! State is stored as JSON files in `~/.cache/foundation_testbed/state/`.
//! Each VM profile has a `<profile-name>.json` file.
//!
//! On load, stale PIDs are detected and cleaned up.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::vms::config::{Result, TestbedError};
use crate::vms::providers::ProviderId;

/// Persistent state for a running (or previously running) VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmState {
    /// Profile name (e.g. "windows-build").
    pub profile_name: String,
    /// Path to the disk image (qcow2 for QEMU, .utm bundle path for UTM).
    pub disk_path: String,
    /// PID of the VM process (Some for QEMU, None for UTM).
    pub pid: Option<u32>,
    /// Provider that launched this VM.
    pub provider_id: ProviderId,
    /// Provider-specific VM identifier (PID for QEMU, UUID for UTM).
    pub provider_internal_id: String,
    /// Path to the monitor socket (QEMU only; empty for UTM).
    pub monitor_socket: String,
    /// Resolved SSH port.
    pub ssh_port: u16,
    /// Resolved WinRM port.
    pub winrm_port: Option<u16>,
    /// Resolved RDP port.
    pub rdp_port: Option<u16>,
    /// Resolved VNC port.
    pub vnc_port: u16,
    /// Whether the VM has been bootstrapped.
    pub bootstrapped: bool,
    /// When the VM state was created.
    pub created_at: String,
}

/// Save VM state to disk.
pub fn save(state: &VmState) -> Result<()> {
    let path = state_path(&state.profile_name);
    let json = serde_json::to_string_pretty(state).map_err(|e| TestbedError::Qcow2Error {
        message: format!("serializing state: {e}"),
    })?;

    std::fs::write(&path, json).map_err(|e| TestbedError::Qcow2Error {
        message: format!("writing state to {path:?}: {e}"),
    })?;

    Ok(())
}

/// Load VM state from disk.
///
/// Performs stale PID detection: if the process is gone, marks the VM
/// as stopped (sets PID to None) and saves the cleaned state.
pub fn load(name: &str) -> Result<VmState> {
    let path = state_path(name);
    if !path.exists() {
        return Err(TestbedError::VmNotRunning {
            name: name.to_string(),
        });
    }

    let json = std::fs::read_to_string(&path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading state from {path:?}: {e}"),
    })?;

    let mut state: VmState = serde_json::from_str(&json).map_err(|e| TestbedError::Qcow2Error {
        message: format!("parsing state: {e}"),
    })?;

    // Stale PID detection
    if let Some(pid) = state.pid
        && !is_process_alive(pid) {
            state.pid = None; // Process is gone
            // Save cleaned state
            save(&state)?;
        }

    Ok(state)
}

/// Check if a VM has a state file.
pub fn exists(name: &str) -> bool {
    state_path(name).exists()
}

/// Delete VM state file.
pub fn delete(name: &str) -> Result<()> {
    let path = state_path(name);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| TestbedError::Qcow2Error {
            message: format!("deleting state {path:?}: {e}"),
        })?;
    }
    Ok(())
}

/// List all known VMs.
///
/// Returns (profile_name, state) for each state file.
pub fn list() -> Result<Vec<(String, VmState)>> {
    let dir = state_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut vms = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading state dir {dir:?}: {e}"),
    })? {
        let entry = entry.map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading state entry: {e}"),
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(json) = std::fs::read_to_string(&path)
                && let Ok(state) = serde_json::from_str::<VmState>(&json) {
                    vms.push((state.profile_name.clone(), state));
                }
    }

    Ok(vms)
}

/// Check if a VM is currently running (has a live PID).
pub fn is_running(name: &str) -> bool {
    if let Ok(state) = load(name)
        && let Some(pid) = state.pid {
            return is_process_alive(pid);
        }
    false
}

/// Build a VmState from runtime info.
pub fn from_runtime(
    profile_name: &str,
    disk_path: &std::path::Path,
    provider_id: ProviderId,
    provider_internal_id: &str,
    ssh_port: u16,
    winrm_port: Option<u16>,
    rdp_port: Option<u16>,
    vnc_port: u16,
    bootstrapped: bool,
) -> VmState {
    VmState {
        profile_name: profile_name.to_string(),
        disk_path: disk_path.to_string_lossy().to_string(),
        pid: None, // Provider sets if applicable
        provider_id,
        provider_internal_id: provider_internal_id.to_string(),
        monitor_socket: String::new(),
        ssh_port,
        winrm_port,
        rdp_port,
        vnc_port,
        bootstrapped,
        created_at: format!("{}T{}",
            chrono::Utc::now().date_naive(),
            chrono::Utc::now().time()
        ),
    }
}

/// Build a VmState from QEMU runtime info (legacy, prefer from_runtime).
pub fn from_qemu(
    profile_name: &str,
    disk_path: &std::path::Path,
    pid: u32,
    monitor_path: &std::path::Path,
    ssh_port: u16,
    winrm_port: Option<u16>,
    rdp_port: Option<u16>,
    vnc_port: u16,
    bootstrapped: bool,
) -> VmState {
    VmState {
        profile_name: profile_name.to_string(),
        disk_path: disk_path.to_string_lossy().to_string(),
        pid: Some(pid),
        provider_id: crate::vms::providers::ProviderId::Qemu,
        provider_internal_id: pid.to_string(),
        monitor_socket: monitor_path.to_string_lossy().to_string(),
        ssh_port,
        winrm_port,
        rdp_port,
        vnc_port,
        bootstrapped,
        created_at: format!("{}T{}",
            chrono::Utc::now().date_naive(),
            chrono::Utc::now().time()
        ),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Path to a profile's state file.
fn state_path(name: &str) -> PathBuf {
    state_dir().join(format!("{name}.json"))
}

/// State directory path.
pub fn state_dir() -> PathBuf {
    crate::vms::config::state_dir()
}

/// Check if a process is alive via kill(pid, 0).
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

