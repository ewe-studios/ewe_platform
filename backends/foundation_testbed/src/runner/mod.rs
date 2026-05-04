//! Runner utilities — launch binaries, capture screenshots, tail logs, transfer files.
//!
//! Post-build utilities for testing and observing applications inside VMs.

use std::path::Path;

use crate::config::{GuestOs, Result, TestbedError, VmProfile};
use crate::ssh::VmSession;

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
        GuestOs::Windows => run_windows(session, &binary)?,
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

    crate::ssh::exec(session, &format!("pkill -f '{bin_name}' 2>/dev/null || true"))?;
    crate::ssh::exec(session, "pkill Xvfb 2>/dev/null || true")?;
    crate::ssh::exec(session, "pkill openbox 2>/dev/null || true")?;

    // Create log directory
    crate::ssh::exec(session, "mkdir -p ~/.testbed-run")?;

    // Start Xvfb
    crate::ssh::exec(
        session,
        "setsid -f Xvfb :99 -screen 0 1280x800x24 -nolisten tcp > ~/.testbed-run/xvfb.log 2>&1",
    )?;

    // Start openbox
    crate::ssh::exec(
        session,
        "sleep 1 && DISPLAY=:99 setsid -f openbox --replace > ~/.testbed-run/openbox.log 2>&1",
    )?;

    // Launch the application
    crate::ssh::exec(
        session,
        &format!("sleep 1 && DISPLAY=:99 setsid -f {binary} > ~/.testbed-run/run.log 2>&1"),
    )?;

    Ok(())
}

/// Launch a binary on Windows via Start-Process.
fn run_windows(session: &mut VmSession, binary: &str) -> Result<()> {
    let escaped = binary.replace("'", "''");
    let script = format!(
        r#"
$run_dir = "$env:USERPROFILE\.testbed-run"
if (-not (Test-Path $run_dir)) {{ mkdir $run_dir -Force }}

# Kill prior instances
$prior = Get-Process -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq '{escaped}' }}
if ($prior) {{ $prior | Stop-Process -Force }}

# Launch with output redirection
Start-Process -FilePath '{escaped}' -RedirectStandardOutput "$run_dir\run.log" -RedirectStandardError "$run_dir\run-error.log"
"#
    );
    crate::ssh::exec(session, &script)?;
    Ok(())
}

/// Auto-detect the compiled binary on the VM.
///
/// Scans common target directories for the most recent executable.
fn auto_detect_bin(session: &mut VmSession, profile: &VmProfile) -> Result<String> {
    let target = crate::build::target_triple(profile.os);
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
        if let Ok(output) = crate::ssh::exec(session, &check_cmd)
            && output.trim() == "EXISTS" {
                // Find the most recent executable file
                let find_cmd = format!(
                    "find {dir} -maxdepth 1 -type f -name '*{ext}' -printf '%T@ %p\\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-"
                );
                if let Ok(output) = crate::ssh::exec(session, &find_cmd) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_detect_search_paths() {
        let target = crate::build::target_triple(GuestOs::Linux);
        assert!(target.contains("aarch64"));
    }
}
