//! Disk shrink & optimization before export.
//!
//! Performs fstrim, zero-fill, and qemu-img compress to produce a smaller
//! exported qcow2 file.

use std::path::Path;
use std::process::Command;

use crate::vms::config::{GuestOs, Result, TestbedError};
use crate::vms::ssh::VmSession;

/// Shrink a VM disk before export by zero-filling free space and compressing.
///
/// This must be called while the VM is running (for fstrim/zero-fill),
/// then the VM should be stopped before calling `compress_qcow2`.
pub fn shrink_disk(session: &mut VmSession, os: GuestOs) -> Result<()> {
    match os {
        GuestOs::Linux | GuestOs::MacOS => linux_shrink(session)?,
        GuestOs::Windows => windows_shrink(session)?,
    }
    Ok(())
}

fn linux_shrink(session: &mut VmSession) -> Result<()> {
    // Step 1: fstrim
    crate::vms::ssh::exec(session, "sudo fstrim -v /").map_err(|e| TestbedError::Qcow2Error {
        message: format!("fstrim failed: {e}"),
    })?;

    // Step 2: zero-fill free space (creates a large file of zeros, then deletes it)
    // This makes the qcow2 sparse and compressible.
    let result = crate::vms::ssh::exec(
        session,
        "sudo dd if=/dev/zero of=/.zero_fill bs=1M status=progress 2>/dev/null; sudo rm -f /.zero_fill",
    ).map_err(|e| TestbedError::Qcow2Error {
        message: format!("zero-fill failed: {e}"),
    })?;
    // dd may exit non-zero if disk is full (expected at end of zero-fill)
    let _ = result;

    Ok(())
}

fn windows_shrink(session: &mut VmSession) -> Result<()> {
    // Use PowerShell Optimize-Volume for TRIM/Defrag
    let script = r#"Optimize-Volume -DriveLetter C -Trim -Verbose"#;
    let (output, _) = crate::vms::ssh::exec_ps_windows(session, script).map_err(|e| TestbedError::Qcow2Error {
        message: format!("Windows trim failed: {e}"),
    })?;
    let _ = output;

    // Use sdelete if available, otherwise skip zero-fill
    let script = r#"if (Get-Command sdelete64 -ErrorAction SilentlyContinue) { sdelete64.exe -accepteula -z C:\ } else { Write-Host 'sdelete not installed, skipping zero-fill' }"#;
    let (output, _) = crate::vms::ssh::exec_ps_windows(session, script).map_err(|e| TestbedError::Qcow2Error {
        message: format!("Windows zero-fill failed: {e}"),
    })?;
    let _ = output;

    Ok(())
}

/// Compress a qcow2 image using `qemu-img convert -O qcow2 -c`.
///
/// Reads `src` and writes a compressed version to `dest`.
pub fn compress_qcow2(src: &Path, dest: &Path) -> Result<()> {
    ensure_qemu_img()?;

    let status = Command::new("qemu-img")
        .args(["convert", "-O", "qcow2", "-c"])
        .arg(src)
        .arg(dest)
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning qemu-img: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("qemu-img convert failed (exit {})", status),
        });
    }

    Ok(())
}

/// Get the size comparison between original and compressed qcow2.
pub fn size_comparison(original: &Path, compressed: &Path) -> Result<(u64, u64, f64)> {
    let orig_meta = std::fs::metadata(original).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading original: {e}"),
    })?;
    let comp_meta = std::fs::metadata(compressed).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading compressed: {e}"),
    })?;

    let orig = orig_meta.len();
    let comp = comp_meta.len();
    let ratio = if orig > 0 { (comp as f64 / orig as f64) * 100.0 } else { 0.0 };

    Ok((orig, comp, ratio))
}

fn ensure_qemu_img() -> Result<()> {
    Command::new("qemu-img")
        .arg("--version")
        .output()
        .map_err(|_| TestbedError::QemuImgNotFound {
            install_cmd: "apt install qemu-utils / dnf install qemu-img / brew install qemu".to_string(),
        })?;
    Ok(())
}

