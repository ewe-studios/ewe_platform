//! .utm bundle config.plist creation and parsing.
//!
//! UTM VMs are stored as `.utm` bundles (directories) containing:
//! - `config.plist` — VM configuration (CPU, memory, disks, network, etc.)
//! - Disk images (typically `.qcow2` or `.img`)
//!
//! This module handles creating/modifying the config.plist for new VMs.

use std::path::{Path, PathBuf};

use crate::config::{Result, TestbedError, VmProfile};

/// Plist key path for VM name.
const NAME_KEY: &str = "Name";

/// Plist key path for UTM version.
const UTM_VERSION_KEY: &str = "UTMVersion";

/// Run PlistBuddy to read a value from a plist.
fn plist_get(plist_path: &Path, key: &str) -> Result<String> {
    let output = std::process::Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", &format!("Print :{key}"), plist_path.to_str().unwrap()])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("PlistBuddy read failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "PlistBuddy read {key} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run PlistBuddy to set a value in a plist.
fn plist_set(plist_path: &Path, key: &str, value: &str) -> Result<()> {
    let output = std::process::Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            &format!("Set :{key} {value}"),
            plist_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("PlistBuddy set failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "PlistBuddy set {key} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(())
}

/// Read the VM name from a .utm bundle's config.plist.
pub fn get_vm_name(bundle_path: &Path) -> Result<String> {
    let plist = config_plist_path(bundle_path);
    plist_get(&plist, NAME_KEY)
}

/// Set the VM name in a .utm bundle's config.plist.
pub fn set_vm_name(bundle_path: &Path, name: &str) -> Result<()> {
    let plist = config_plist_path(bundle_path);
    plist_set(&plist, NAME_KEY, &quoted_plist_string(name))
}

/// Read the UTM version from a .utm bundle's config.plist.
pub fn get_utm_version(bundle_path: &Path) -> Result<String> {
    let plist = config_plist_path(bundle_path);
    plist_get(&plist, UTM_VERSION_KEY)
}

/// Get the path to config.plist inside a .utm bundle.
pub fn config_plist_path(bundle_path: &Path) -> PathBuf {
    bundle_path.join("config.plist")
}

/// Check if a path looks like a valid .utm bundle.
pub fn is_utm_bundle(path: &Path) -> bool {
    path.is_dir()
        && path.extension().and_then(|e| e.to_str()) == Some("utm")
        && config_plist_path(path).exists()
}

/// Configure CPU cores in the config.plist.
pub fn set_cpu_cores(bundle_path: &Path, cores: u32) -> Result<()> {
    let plist = config_plist_path(bundle_path);
    plist_set(&plist, "CPUCount", &cores.to_string())
}

/// Configure memory (RAM) in the config.plist.
pub fn set_memory_mb(bundle_path: &Path, mb: u32) -> Result<()> {
    let plist = config_plist_path(bundle_path);
    plist_set(&plist, "MemorySize", &mb.to_string())
}

/// Quote a string value for PlistBuddy Set command.
///
/// PlistBuddy expects string values in the format `"value"` (with quotes).
pub fn quoted_plist_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Apply profile resource settings to a .utm bundle's config.plist.
///
/// Sets VM name, CPU cores, and memory based on the VmProfile.
pub fn apply_profile(bundle_path: &Path, profile: &VmProfile) -> Result<()> {
    set_vm_name(bundle_path, profile.name)?;

    if profile.cpu_cores > 0 {
        set_cpu_cores(bundle_path, profile.cpu_cores)?;
    }

    if profile.memory_mib > 0 {
        set_memory_mb(bundle_path, profile.memory_mib)?;
    }

    Ok(())
}

