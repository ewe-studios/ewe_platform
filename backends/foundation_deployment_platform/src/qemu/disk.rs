//! QEMU disk image operations via `qemu-img`.

use std::path::Path;
use std::process::Command;

use crate::config::{Result, TestbedError};

/// Create a new qcow2 disk image.
///
/// # Errors
///
/// Returns an error if `qemu-img` cannot be found, cannot be spawned, or fails.
///
/// # Panics
///
/// Panics if `path` is not valid UTF-8.
pub fn create(path: &Path, size_gb: u32) -> Result<()> {
    let qemu_img = find_qemu_img()?;
    let status = Command::new(qemu_img)
        .args([
            "create",
            "-f",
            "qcow2",
            path.to_str().unwrap(),
            &format!("{size_gb}G"),
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;
    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("qemu-img create failed with exit code {status}"),
        });
    }
    Ok(())
}

/// Get information about a qcow2 image.
///
/// # Errors
///
/// Returns an error if `qemu-img` cannot be found, cannot be spawned, fails, or
/// if its JSON output cannot be parsed.
///
/// # Panics
///
/// Panics if `path` is not valid UTF-8.
pub fn info(path: &Path) -> Result<DiskInfo> {
    let qemu_img = find_qemu_img()?;
    let output = Command::new(qemu_img)
        .args(["info", "--output=json", path.to_str().unwrap()])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;
    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "qemu-img info failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    let json = String::from_utf8_lossy(&output.stdout);
    parse_disk_info(&json)
}

/// Check disk integrity.
///
/// # Errors
///
/// Returns an error if `qemu-img` cannot be found, cannot be spawned, or fails.
///
/// # Panics
///
/// Panics if `path` is not valid UTF-8.
pub fn check(path: &Path) -> Result<()> {
    let qemu_img = find_qemu_img()?;
    let status = Command::new(qemu_img)
        .args(["check", path.to_str().unwrap()])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;
    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("qemu-img check failed with exit code {status}"),
        });
    }
    Ok(())
}

/// Resize a qcow2 image by adding N GB.
///
/// # Errors
///
/// Returns an error if `qemu-img` cannot be found, cannot be spawned, or fails.
///
/// # Panics
///
/// Panics if `path` is not valid UTF-8.
pub fn resize(path: &Path, plus_gb: u32) -> Result<()> {
    let qemu_img = find_qemu_img()?;
    let status = Command::new(qemu_img)
        .args(["resize", path.to_str().unwrap(), &format!("+{plus_gb}G")])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;
    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("qemu-img resize failed with exit code {status}"),
        });
    }
    Ok(())
}

/// Information extracted from `qemu-img info --output=json`.
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub format: String,
    pub virtual_size_bytes: u64,
    pub actual_size_bytes: u64,
}

/// Parse the JSON output of `qemu-img info` into a `DiskInfo`.
///
/// # Errors
///
/// Returns an error if the JSON is invalid or is missing `virtual-size`.
pub fn parse_disk_info(json: &str) -> Result<DiskInfo> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| TestbedError::Qcow2Error {
            message: format!("parsing qemu-img info JSON: {e}"),
        })?;

    let virtual_size = parsed
        .get("virtual-size")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| TestbedError::Qcow2Error {
            message: "virtual-size missing from qemu-img info".to_string(),
        })?;

    let actual_size = parsed
        .get("actual-size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let format = parsed
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(DiskInfo {
        format,
        virtual_size_bytes: virtual_size,
        actual_size_bytes: actual_size,
    })
}

impl DiskInfo {
    #[must_use]
    pub fn virtual_size_gb(&self) -> f64 {
        // size in bytes to f64 for display; precision loss is acceptable.
        #[allow(clippy::cast_precision_loss)]
        {
            self.virtual_size_bytes as f64 / 1_073_741_824.0
        }
    }

    #[must_use]
    pub fn actual_size_mb(&self) -> f64 {
        // size in bytes to f64 for display; precision loss is acceptable.
        #[allow(clippy::cast_precision_loss)]
        {
            self.actual_size_bytes as f64 / 1_048_576.0
        }
    }
}

/// Find `qemu-img` on PATH.
///
/// # Errors
///
/// Returns an error if `qemu-img` cannot be found on PATH or in the known
/// install locations.
pub fn find_qemu_img() -> Result<std::path::PathBuf> {
    if let Ok(p) = which::which("qemu-img") {
        return Ok(p);
    }
    let candidates = [
        "/opt/homebrew/bin/qemu-img",
        "/usr/local/bin/qemu-img",
        "/run/current-system/sw/bin/qemu-img",
    ];
    for c in &candidates {
        let p = std::path::PathBuf::from(c);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(TestbedError::QemuImgNotFound {
        install_cmd: "mise install qemu".to_string(),
    })
}
