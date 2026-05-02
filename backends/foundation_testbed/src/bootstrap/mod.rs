//! Bootstrap orchestrator — installs development tools inside guest VMs.
//!
//! Two-tier mise strategy:
//! - Tier 1: Bootstrap mise.toml (hardcoded, installs rust, nu, tauri-cli, etc.)
//! - Tier 2: Project mise.toml (user's project, installed at build time)
//!
//! After bootstrap, nushell is the default execution shell.

use crate::config::{GuestOs, Result, VmProfile};

use crate::ssh::VmSession;
use crate::winrm::WinRM;

pub mod boot_wait;
pub mod linux;
pub mod windows;

/// Marker files that indicate a VM has been bootstrapped.
const WINDOWS_MARKER: &str = "C:\\Users\\vagrant\\.testbed-bootstrapped";
const LINUX_MARKER: &str = "/home/vagrant/.testbed-bootstrapped";

/// Check if a profile's VM has already been bootstrapped.
pub fn is_bootstrapped(profile: &VmProfile) -> bool {
    match profile.os {
        GuestOs::Windows => {
            // Check via WinRM if the marker file exists
            if let Ok(winrm) = WinRM::from_profile(profile) {
                let result = winrm.run_ps(&format!(
                    "if (Test-Path '{WINDOWS_MARKER}') {{ 'YES' }} else {{ 'NO' }}"
                ));
                if let Ok(cmd) = result {
                    return cmd.stdout.trim() == "YES";
                }
            }
            false
        }
        GuestOs::Linux => {
            // Check via SSH if the marker file exists
            if let Ok(mut session) = crate::ssh::connect(profile) {
                let result = crate::ssh::exec(&mut session, &format!("[ -f {LINUX_MARKER} ] && echo YES || echo NO"));
                if let Ok(output) = result {
                    return output.trim() == "YES";
                }
            }
            false
        }
    }
}

/// Bootstrap a VM with development tools.
///
/// This is the main entry point. It delegates to OS-specific bootstrap
/// modules and verifies the result.
pub fn bootstrap(profile: &VmProfile, session: &mut VmSession) -> Result<()> {
    if is_bootstrapped(profile) {
        return Ok(()); // Skip — already bootstrapped
    }

    match profile.os {
        GuestOs::Windows => {
            let winrm = WinRM::from_profile(profile)?;
            windows::bootstrap_windows(profile, &winrm, session)?;
        }
        GuestOs::Linux => {
            linux::bootstrap_linux(profile, session)?;
        }
    }

    // Verify bootstrap succeeded
    if !is_bootstrapped(profile) {
        return Err(crate::config::TestbedError::BootstrapFailed {
            step: "verification".to_string(),
            message: "bootstrap marker not found after completion".to_string(),
        });
    }

    Ok(())
}

/// The bootstrap mise.toml content (Tier 1).
///
/// Installed during initial VM setup to provide the dev toolchain.
pub const BOOTSTRAP_MISE_TOML: &str = r#"[tools]
rust = "stable"
nu = "latest"
"cargo:cargo-binstall" = "latest"
"cargo:sccache" = "latest"
"cargo:tauri-cli" = "2"

[settings]
cargo_binstall = true
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn test_bootstrap_mise_toml_is_valid() {
        // Verify it at least parses as a reasonable TOML string
        let toml = super::BOOTSTRAP_MISE_TOML;
        assert!(toml.contains("[tools]"));
        assert!(toml.contains("rust = \"stable\""));
        assert!(toml.contains("nu = \"latest\""));
        assert!(toml.contains("cargo:tauri-cli"));
    }
}
