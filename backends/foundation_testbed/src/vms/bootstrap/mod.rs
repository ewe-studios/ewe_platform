//! Bootstrap orchestrator — installs development tools inside guest VMs.
//!
//! Two-tier mise strategy:
//! - Tier 1: Bootstrap mise.toml (hardcoded, installs rust, nu, tauri-cli, etc.)
//! - Tier 2: Project mise.toml (user's project, installed at build time)
//!
//! After bootstrap, nushell is the default execution shell.

use std::thread;
use std::time::Duration;

use crate::vms::config::{GuestOs, Result, VmProfile};
use crate::vms::ssh::VmSession;
use crate::vms::winrm::elevated::ProgressCallback;
use crate::vms::winrm::WinRM;

pub mod boot_wait;
pub mod linux;
pub mod logger;
pub mod macos;
pub mod windows;

pub use logger::BootstrapLogger;

/// Marker files that indicate a VM has been bootstrapped.
const WINDOWS_MARKER: &str = "C:\\Users\\vagrant\\.testbed-bootstrapped";
const LINUX_MARKER: &str = "/home/vagrant/.testbed-bootstrapped";
const MACOS_MARKER: &str = "/Users/vagrant/.testbed-bootstrapped";

/// Check if a profile's VM has already been bootstrapped.
pub fn is_bootstrapped(profile: &VmProfile) -> bool {
    match profile.os {
        GuestOs::Windows => {
            // Try WinRM first, fall back to SSH (WinRM may be down after nushell setup)
            if let Ok(winrm) = WinRM::from_profile(profile) {
                let result = winrm.run_ps_quiet(&format!(
                    "if (Test-Path '{WINDOWS_MARKER}') {{ 'YES' }} else {{ 'NO' }}"
                ));
                if let Ok(cmd) = result {
                    return cmd.stdout.trim() == "YES";
                }
            }
            // Fall back to SSH
            if let Ok(mut session) = crate::vms::ssh::connect(profile) {
                let result = crate::vms::ssh::exec_ps_windows(&mut session,
                    &format!("if (Test-Path '{WINDOWS_MARKER}') {{ 'YES' }} else {{ 'NO' }}")
                );
                if let Ok((output, _)) = result {
                    return output.trim() == "YES";
                }
            }
            false
        }
        GuestOs::Linux => {
            if let Ok(mut session) = crate::vms::ssh::connect(profile) {
                let result = crate::vms::ssh::exec(&mut session, &format!("[ -f {LINUX_MARKER} ] && echo YES || echo NO"));
                if let Ok(output) = result {
                    return output.trim() == "YES";
                }
            }
            false
        }
        GuestOs::MacOS => {
            if let Ok(mut session) = crate::vms::ssh::connect(profile) {
                let result = crate::vms::ssh::exec(&mut session, &format!("[ -f {MACOS_MARKER} ] && echo YES || echo NO"));
                if let Ok(output) = result {
                    return output.trim() == "YES";
                }
            }
            false
        }
    }
}

/// Bootstrap a VM with development tools, writing progress to a log file.
///
/// Log file is written to `$PWD/.testbed/<vm-name>/bootstrap.log`.
///
/// For Windows, this handles the two-phase flow automatically:
/// 1. WinRM phase — installs OpenSSH, configures keys/autologin
/// 2. Waits for SSH to become available
/// 3. SSH phase — installs mise, build tools, runtimes
///
/// For Linux/macOS, runs SSH-only bootstrap directly.
pub fn bootstrap(profile: &VmProfile, session: &mut VmSession, logger: &BootstrapLogger, progress: ProgressCallback<'_>) -> Result<()> {
    if is_bootstrapped(profile) {
        logger.message("already bootstrapped, skipping");
        return Ok(());
    }

    match profile.os {
        GuestOs::Windows => {
            let winrm = WinRM::from_profile(profile)?;

            // Phase 1: WinRM-only (installs OpenSSH)
            if !is_bootstrapped(profile) {
                windows::bootstrap_windows_winrm_phase(profile, &winrm, logger, progress)?;
            }

            // Wait for SSH to come up
            thread::sleep(Duration::from_secs(10));
            let deadline = std::time::Instant::now() + Duration::from_secs(120);
            loop {
                if std::time::Instant::now() > deadline {
                    return Err(crate::vms::config::TestbedError::BootstrapFailed {
                        step: "wait for SSH".to_string(),
                        message: "timed out waiting for SSH after WinRM bootstrap".to_string(),
                    });
                }
                if crate::vms::ssh::connect(profile).is_ok() {
                    break;
                }
                thread::sleep(Duration::from_secs(5));
            }

            // Reconnect session for SSH phase
            *session = crate::vms::ssh::connect(profile)?;

            // Phase 2: SSH-required (installs dev tools)
            windows::bootstrap_windows_ssh_phase(profile, &winrm, session, logger, progress, false)?;
        }
        GuestOs::Linux => {
            linux::bootstrap_linux(profile, session, logger)?;
        }
        GuestOs::MacOS => {
            bootstrap_macos(profile, session, logger)?;
        }
    }

    // Verify bootstrap succeeded
    if !is_bootstrapped(profile) {
        return Err(crate::vms::config::TestbedError::BootstrapFailed {
            step: "verification".to_string(),
            message: "bootstrap marker not found after completion".to_string(),
        });
    }

    Ok(())
}

/// Bootstrap a macOS VM with development tools via SSH.
fn bootstrap_macos(profile: &crate::vms::config::VmProfile, session: &mut VmSession, logger: &BootstrapLogger) -> Result<()> {
    macos::bootstrap_macos(profile, session, logger)
}

/// The bootstrap mise.toml content (Tier 1).
///
/// Installed during initial VM setup to provide the dev toolchain.
pub const BOOTSTRAP_MISE_TOML: &str = include_str!("../../../scripts/bootstrap_mise.toml");

