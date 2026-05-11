//! utmctl CLI wrapper — VM lifecycle operations for UTM on macOS.
//!
//! Uses `/Applications/UTM.app/Contents/MacOS/utmctl` for list, start, stop.
//! Version check via `PlistBuddy` on UTM's Info.plist.

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{Result, TestbedError};

/// Path to utmctl binary.
pub const UTMCTL: &str = "/Applications/UTM.app/Contents/MacOS/utmctl";

/// Minimum tested UTM version.
pub const MIN_UTM_VERSION: &str = "4.6.5";

/// A VM entry from `utmctl list`.
#[derive(Debug, Clone)]
pub struct VmEntry {
    pub uuid: String,
    pub status: String,
    pub name: String,
}

/// Check if UTM is installed and meets the minimum version.
pub fn check_utm_version() -> Result<Option<String>> {
    let plist_path = "/Applications/UTM.app/Contents/Info.plist";
    if !std::path::Path::new(plist_path).exists() {
        return Ok(None);
    }

    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleShortVersionString", plist_path])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("checking UTM version: {e}"),
        })?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let warning = if version_is_older(&version, MIN_UTM_VERSION) {
            Some(format!(
                "UTM {version} found, minimum is {MIN_UTM_VERSION} — update recommended"
            ))
        } else {
            None
        };
        Ok(warning)
    } else {
        Ok(None)
    }
}

/// Check UTM.app version, return it if installed.
pub fn installed_utm_version() -> Option<String> {
    let plist = "/Applications/UTM.app/Contents/Info.plist";
    let out = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleShortVersionString", plist])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Check that UTM.app is responsive (try listing VMs).
pub fn ensure_utm() -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    // Check utmctl exists
    if !std::path::Path::new(UTMCTL).exists() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "UTM not installed at /Applications/UTM.app. Install via: brew install --cask utm"
            ),
        });
    }

    // Check that utmctl is responsive
    let output = Command::new(UTMCTL)
        .args(["list"])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("utmctl list failed: {e}"),
        })?;

    if !output.status.success() {
        // UTM may not be running — try launching it
        eprintln!("  UTM.app not responsive — launching...");
        Command::new("open")
            .args(["-g", "/Applications/UTM.app"])
            .status()
            .map_err(|e| TestbedError::Qcow2Error {
                message: format!("launching UTM.app: {e}"),
            })?;

        // Wait up to 30s for UTM to become responsive
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            thread::sleep(Duration::from_secs(1));
            if Command::new(UTMCTL)
                .arg("list")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                eprintln!("  UTM ready");
                break;
            }
        }

        // Verify
        let output = Command::new(UTMCTL)
            .args(["list"])
            .output()
            .map_err(|e| TestbedError::Qcow2Error {
                message: format!("utmctl list failed after launch: {e}"),
            })?;

        if !output.status.success() {
            return Err(TestbedError::Qcow2Error {
                message: "UTM did not become ready after 30s".to_string(),
            });
        }
    }

    // Version check (non-fatal warning)
    if let Some(warning) = check_utm_version()? {
        warnings.push(warning);
    }

    Ok(warnings)
}

/// List all VMs known to UTM.
pub fn list_vms() -> Result<Vec<VmEntry>> {
    let output = Command::new(UTMCTL)
        .args(["list"])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("utmctl list failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "utmctl list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines().skip(1) {
        // Format: UUID\tSTATUS\tNAME
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            entries.push(VmEntry {
                uuid: parts[0].trim().to_string(),
                status: parts[1].trim().to_string(),
                name: parts[2].trim().to_string(),
            });
        }
    }

    Ok(entries)
}

/// Find a VM by name.
pub fn find_vm_by_name(name: &str) -> Result<Option<VmEntry>> {
    let vms = list_vms()?;
    Ok(vms.into_iter().find(|vm| vm.name == name))
}

/// Read the Name from a .utm bundle's config.plist.
pub fn get_vm_name(bundle_path: &std::path::Path) -> Result<String> {
    let plist = bundle_path.join("config.plist");
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :Name", plist.to_str().unwrap()])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("PlistBuddy read failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "PlistBuddy read Name failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Start a VM by name, with retry logic (up to 3 attempts).
pub fn start_vm(name: &str) -> Result<()> {
    // Already running?
    if let Some(e) = list_vms()?.into_iter().find(|e| e.name == name) {
        if e.status == "started" {
            return Ok(());
        }
    }

    for attempt in 1..=3 {
        let output = Command::new(UTMCTL)
            .args(["start", name])
            .output()
            .map_err(|e| TestbedError::Qcow2Error {
                message: format!("utmctl start {name} failed: {e}"),
            })?;

        if output.status.success() {
            return Ok(());
        }
        if attempt < 3 {
            thread::sleep(Duration::from_secs(5));
        }
    }

    Err(TestbedError::Qcow2Error {
        message: format!("failed to start '{name}' after 3 attempts"),
    })
}

/// Stop a VM by name. No-op if not running.
pub fn stop_vm(name: &str) -> Result<()> {
    let running = list_vms()?
        .into_iter()
        .any(|e| e.name == name && e.status == "started");
    if !running {
        return Ok(());
    }

    let output = Command::new(UTMCTL)
        .args(["stop", name])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("utmctl stop {name} failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "utmctl stop {name} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    thread::sleep(Duration::from_secs(5));
    Ok(())
}

/// Delete a VM by UUID (used for orphan cleanup).
pub fn delete_vm(uuid: &str) -> Result<()> {
    let output = Command::new(UTMCTL)
        .args(["delete", uuid])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("utmctl delete {uuid} failed: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "utmctl delete {uuid} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(())
}

/// Check if a VM is running.
pub fn is_vm_running(name: &str) -> Result<bool> {
    if let Some(vm) = find_vm_by_name(name)? {
        Ok(vm.status == "started")
    } else {
        Ok(false)
    }
}

/// Wait for a VM UUID to appear in `utmctl list` after import.
pub fn wait_for_vm(uuid: &str, timeout_secs: u64) -> Result<bool> {
    let start = Instant::now();
    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Ok(false);
        }
        if let Ok(vms) = list_vms() {
            if vms.iter().any(|vm| vm.uuid == uuid) {
                return Ok(true);
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}

/// Compare two semantic versions. Returns true if `actual` < `min`.
fn version_is_older(actual: &str, min: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let mut parts = v.split('.').map(|s| s.parse::<u32>().unwrap_or(0));
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    };
    let (a1, a2, a3) = parse(actual);
    let (m1, m2, m3) = parse(min);
    (a1, a2, a3) < (m1, m2, m3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_is_older() {
        assert!(version_is_older("4.6.4", "4.6.5"));
        assert!(version_is_older("4.5.0", "4.6.5"));
        assert!(version_is_older("3.9.9", "4.6.5"));
        assert!(!version_is_older("4.6.5", "4.6.5"));
        assert!(!version_is_older("4.6.6", "4.6.5"));
        assert!(!version_is_older("4.7.0", "4.6.5"));
        assert!(!version_is_older("5.0.0", "4.6.5"));
    }
}
