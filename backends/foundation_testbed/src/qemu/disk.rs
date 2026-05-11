//! QEMU disk image operations via `qemu-img`.

use std::path::Path;
use std::process::Command;

use crate::config::{Result, TestbedError};

/// Create a new qcow2 disk image.
pub fn create(path: &Path, size_gb: u32) -> Result<()> {
    let qemu_img = find_qemu_img()?;
    let status = Command::new(qemu_img)
        .args(["create", "-f", "qcow2", path.to_str().unwrap(), &format!("{size_gb}G")])
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

fn parse_disk_info(json: &str) -> Result<DiskInfo> {
    let parsed: serde_json::Value = serde_json::from_str(json).map_err(|e| {
        TestbedError::Qcow2Error {
            message: format!("parsing qemu-img info JSON: {e}"),
        }
    })?;

    let virtual_size = parsed
        .get("virtual-size")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| TestbedError::Qcow2Error {
            message: "virtual-size missing from qemu-img info".to_string(),
        })?;

    let actual_size = parsed
        .get("actual-size")
        .and_then(|v| v.as_u64())
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
    pub fn virtual_size_gb(&self) -> f64 {
        self.virtual_size_bytes as f64 / 1_073_741_824.0
    }

    pub fn actual_size_mb(&self) -> f64 {
        self.actual_size_bytes as f64 / 1_048_576.0
    }
}

/// Find `qemu-img` on PATH.
fn find_qemu_img() -> Result<std::path::PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_disk_info() {
        let json = r#"{
            "virtual-size": 85899345920,
            "actual-size": 1234567890,
            "format": "qcow2"
        }"#;
        let info = parse_disk_info(json).unwrap();
        assert_eq!(info.format, "qcow2");
        assert_eq!(info.virtual_size_bytes, 85_899_345_920);
        assert!((info.virtual_size_gb() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_qemu_img_not_found_error() {
        let err = find_qemu_img();
        // Should either find qemu-img or return the right error type
        match err {
            Ok(_) => {} // qemu-img is installed, fine
            Err(TestbedError::QemuImgNotFound { .. }) => {} // expected error
            Err(e) => panic!("unexpected error variant: {e}"),
        }
    }
}
