//! Runner utilities — launch binaries, capture screenshots, tail logs, transfer files.
//!
//! Post-build utilities for testing and observing applications inside VMs.

use std::path::Path;

use crate::vms::config::{GuestOs, Result, TestbedError, VmProfile};
use crate::vms::ssh::VmSession;
use crate::vms::winrm::WinRM;

pub mod logs;
pub mod screenshot;
pub mod transfer;
pub mod ui;
pub mod validate;

/// Run result — contains PID of the launched process.
#[derive(Debug)]
pub struct RunResult {
    /// PID of the launched process (if available).
    pub pid: Option<u32>,
    /// Log file path on the VM.
    pub log_path: String,
}

/// Launch a compiled binary inside the VM.
///
/// Sets up Xvfb + openbox on Linux, or Start-Process on Windows.
/// Redirects output to ~/.testbed-run/run.log.
pub fn run_in_vm(
    profile: &VmProfile,
    session: &mut VmSession,
    winrm: Option<&WinRM>,
    bin_path: Option<&str>,
) -> Result<RunResult> {
    // Auto-detect binary if not specified
    let binary = match bin_path {
        Some(p) => p.to_string(),
        None => auto_detect_bin(session, profile)?,
    };

    match profile.os {
        GuestOs::Linux => run_linux(session, &binary)?,
        GuestOs::MacOS => run_linux(session, &binary)?,
        GuestOs::Windows => run_windows(session, winrm, &binary)?,
    }

    Ok(RunResult {
        pid: None, // Would need process tracking to get this
        log_path: match profile.os {
            GuestOs::Linux => "/home/vagrant/.testbed-run/run.log".to_string(),
            GuestOs::MacOS => "/Users/vagrant/.testbed-run/run.log".to_string(),
            GuestOs::Windows => "C:\\Users\\vagrant\\.testbed-run\\run.log".to_string(),
        },
    })
}

/// Launch a binary on Linux with Xvfb + openbox.
fn run_linux(session: &mut VmSession, binary: &str) -> Result<()> {
    // Kill any prior instances
    let bin_name = Path::new(binary)
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();

    crate::vms::ssh::exec(session, &format!("pkill -f '{bin_name}' 2>/dev/null || true"))?;
    crate::vms::ssh::exec(session, "pkill Xvfb 2>/dev/null || true")?;
    crate::vms::ssh::exec(session, "pkill openbox 2>/dev/null || true")?;

    // Create log directory
    crate::vms::ssh::exec(session, "mkdir -p ~/.testbed-run")?;

    // Start Xvfb
    crate::vms::ssh::exec(
        session,
        "setsid -f Xvfb :99 -screen 0 1280x800x24 -nolisten tcp > ~/.testbed-run/xvfb.log 2>&1",
    )?;

    // Start openbox
    crate::vms::ssh::exec(
        session,
        "sleep 1 && DISPLAY=:99 setsid -f openbox --replace > ~/.testbed-run/openbox.log 2>&1",
    )?;

    // Launch the application
    crate::vms::ssh::exec(
        session,
        &format!("sleep 1 && DISPLAY=:99 setsid -f {binary} > ~/.testbed-run/run.log 2>&1"),
    )?;

    Ok(())
}

/// Launch a binary on Windows via interactive desktop session.
///
/// Uses a Scheduled Task with `/RU vagrant /IT` so GUI apps (Tauri, etc.)
/// actually appear on screen. Falls back to headless Start-Process if
/// WinRM is not available.
fn run_windows(session: &mut VmSession, winrm: Option<&WinRM>, binary: &str) -> Result<()> {
    // Try interactive launch first (GUI apps work)
    if let Some(winrm) = winrm {
        crate::vms::bootstrap::windows::run_interactive_windows(winrm, binary, &[])?;
        return Ok(());
    }

    // Fallback: headless (no GUI, but won't crash)
    let escaped = binary.replace("'", "''");
    let script = format!(
        r#"
$run_dir = "$env:USERPROFILE\.testbed-run"
if (-not (Test-Path $run_dir)) {{ mkdir $run_dir -Force }}

$prior = Get-Process -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq '{escaped}' }}
if ($prior) {{ $prior | Stop-Process -Force }}

Start-Process -FilePath '{escaped}' -RedirectStandardOutput "$run_dir\run.log" -RedirectStandardError "$run_dir\run-error.log"
"#
    );
    crate::vms::ssh::exec(session, &script)?;
    Ok(())
}

/// Auto-detect the compiled binary on the VM.
///
/// Scans common target directories for the most recent executable.
fn auto_detect_bin(session: &mut VmSession, profile: &VmProfile) -> Result<String> {
    let target = crate::vms::build::target_triple(profile.os);
    let ext = match profile.os {
        GuestOs::Windows => ".exe",
        GuestOs::Linux | GuestOs::MacOS => "",
    };

    // Candidate directories to probe
    let candidates = [
        format!("~/project/target/{target}/release"),
        format!("~/project/src-tauri/target/{target}/release"),
    ];

    for dir in &candidates {
        // Check if directory exists
        let check_cmd = format!("test -d {dir} && echo EXISTS || echo MISSING");
        if let Ok(output) = crate::vms::ssh::exec(session, &check_cmd)
            && output.trim() == "EXISTS" {
                // Find the most recent executable file
                let find_cmd = format!(
                    "find {dir} -maxdepth 1 -type f -name '*{ext}' -printf '%T@ %p\\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-"
                );
                if let Ok(output) = crate::vms::ssh::exec(session, &find_cmd) {
                    let path = output.trim();
                    if !path.is_empty() {
                        return Ok(path.to_string());
                    }
                }
            }
    }

    Err(TestbedError::ArtifactNotFound {
        path: "no compiled binary found in target/".to_string(),
    })
}

