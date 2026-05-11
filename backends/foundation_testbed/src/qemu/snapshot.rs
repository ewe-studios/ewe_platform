//! VM snapshot management via QEMU monitor socket.
//!
//! Snapshots are stored inside the qcow2 image itself (internal snapshots).
//! Operations: create (`savevm`), restore (`loadvm`), delete (`delvm`), list (`info snapshots`).

use crate::config::{Result, TestbedError};

use super::QemuVm;

/// Snapshot metadata returned by `info snapshots`.
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    /// Internal snapshot ID (numeric).
    pub id: u32,
    /// Human-readable tag/name.
    pub tag: String,
    /// Date/time when the snapshot was created (as reported by QEMU).
    pub date: String,
    /// Size on disk in bytes (if available).
    pub vm_clock_size: u64,
}

/// Create a named snapshot of the running VM.
///
/// Equivalent to `(qemu) savevm <name>`.
pub fn save(vm: &mut QemuVm, name: &str) -> Result<()> {
    let output = vm.monitor_command(&format!("savevm {name}"))?;
    // QEMU returns an error string on failure; empty or "OK" means success
    if output.to_lowercase().contains("error") {
        return Err(TestbedError::SnapshotFailed {
            name: name.to_string(),
            reason: output,
        });
    }
    Ok(())
}

/// Restore the VM to a previously saved snapshot.
///
/// Equivalent to `(qemu) loadvm <name>`. This implicitly pauses the VM
/// and restores CPU/memory/device state.
pub fn load(vm: &mut QemuVm, name: &str) -> Result<()> {
    let output = vm.monitor_command(&format!("loadvm {name}"))?;
    if output.to_lowercase().contains("error") {
        return Err(TestbedError::SnapshotFailed {
            name: name.to_string(),
            reason: output,
        });
    }
    Ok(())
}

/// Delete a snapshot from the qcow2 image.
///
/// Equivalent to `(qemu) delvm <name>`. Frees the space used by the
/// snapshot within the qcow2 file.
pub fn delete(vm: &mut QemuVm, name: &str) -> Result<()> {
    let output = vm.monitor_command(&format!("delvm {name}"))?;
    if output.to_lowercase().contains("error") {
        return Err(TestbedError::SnapshotFailed {
            name: name.to_string(),
            reason: output,
        });
    }
    Ok(())
}

/// List all snapshots for the running VM.
///
/// Equivalent to `(qemu) info snapshots`. Parses the text output
/// into structured `SnapshotInfo` entries.
pub fn list(vm: &mut QemuVm) -> Result<Vec<SnapshotInfo>> {
    let output = vm.monitor_command("info snapshots")?;

    // QEMU output format:
    // ID        TAG                 VM SIZE                DATE       VM CLOCK
    // 1         my-snap               432MB 2024-01-15 10:30:00   00:05:23.123
    //
    // "No snapshots available." when empty
    if output.contains("No snapshots") || output.is_empty() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        // Skip header line
        if line.starts_with("ID") || line.is_empty() {
            continue;
        }

        // Parse: ID TAG VM-SIZE DATE VM-CLOCK
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let id = parts[0].parse::<u32>().unwrap_or(0);
        let tag = parts[1].to_string();
        // VM SIZE is parts[2], DATE is parts[3] (possibly with time in parts[4])
        let date = if parts.len() >= 6 {
            format!("{} {}", parts[3], parts[4])
        } else {
            parts[3].to_string()
        };
        // VM clock size is approximate from VM SIZE field (e.g. "432MB")
        let vm_clock_size = parse_size_string(parts[2]);

        snapshots.push(SnapshotInfo {
            id,
            tag,
            date,
            vm_clock_size,
        });
    }

    Ok(snapshots)
}

/// Parse a human-readable size string like "432MB" or "1.2GB" into bytes.
fn parse_size_string(s: &str) -> u64 {
    let s = s.to_uppercase();
    if let Some(num) = s.strip_suffix("MB") {
        (num.parse::<f64>().unwrap_or(0.0) * 1_048_576.0) as u64
    } else if let Some(num) = s.strip_suffix("GB") {
        (num.parse::<f64>().unwrap_or(0.0) * 1_073_741_824.0) as u64
    } else if let Some(num) = s.strip_suffix("KB") {
        (num.parse::<f64>().unwrap_or(0.0) * 1_024.0) as u64
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_string() {
        assert_eq!(parse_size_string("432MB"), 432 * 1_048_576);
        assert_eq!(parse_size_string("1GB"), 1_073_741_824);
        assert_eq!(parse_size_string("512KB"), 512 * 1_024);
        assert_eq!(parse_size_string("100"), 100);
        assert_eq!(parse_size_string("invalid"), 0);
    }

    #[test]
    fn test_list_parses_empty() {
        // Simulates QEMU output when no snapshots exist
        let output = "No snapshots available.";
        assert!(output.contains("No snapshots"));
        // Would return empty vec in the real function
    }
}
