//! Linux bootstrap via SSH — step-by-step idempotent.

use crate::bootstrap::{BOOTSTRAP_MISE_TOML, logger};
use crate::bootstrap::BootstrapLogger;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;

const INSTALL_SYSTEM_DEPS_SH: &str = include_str!("../../scripts/linux/install_system_deps.sh");
const INSTALL_MISE_SH: &str = include_str!("../../scripts/linux/install_mise.sh");
const ACTIVATE_MISE_BASHRC_SH: &str = include_str!("../../scripts/linux/activate_mise_bashrc.sh");
const INSTALL_CARGO_BINSTALL_SH: &str = include_str!("../../scripts/linux/install_cargo_binstall.sh");
const CONFIGURE_MISE_CARGO_BINSTALL_SH: &str = include_str!("../../scripts/linux/configure_mise_cargo_binstall.sh");
const INSTALL_TOOLS_MISE_SH: &str = include_str!("../../scripts/linux/install_tools_mise.sh");
const SET_NUSHELL_DEFAULT_SHELL_SH: &str = include_str!("../../scripts/linux/set_nushell_default_shell.sh");
const SETUP_SSH_KEYS_SH: &str = include_str!("../../scripts/linux/setup_ssh_keys.sh");
const SETUP_PROJECT_MOUNT_SH: &str = include_str!("../../scripts/linux/setup_project_mount.sh");
const INSTALL_GUI_SH: &str = include_str!("../../scripts/linux/install_gui.sh");

/// Detect Linux distro and return package manager type
fn detect_distro(session: &mut VmSession) -> Result<String> {
    let os_release = crate::ssh::exec(session, "cat /etc/os-release 2>/dev/null || echo 'ID=unknown'")?;

    if os_release.contains("ID=ubuntu") || os_release.contains("ID=debian") || os_release.contains("ID_LIKE=debian") {
        Ok("debian".to_string())
    } else if os_release.contains("ID=arch") || os_release.contains("ID_LIKE=arch") || os_release.contains("ID=manjaro") {
        Ok("arch".to_string())
    } else if crate::ssh::exec(session, "command -v apt-get >/dev/null 2>&1 && echo yes").unwrap_or_default().contains("yes") {
        Ok("debian".to_string())
    } else if crate::ssh::exec(session, "command -v pacman >/dev/null 2>&1 && echo yes").unwrap_or_default().contains("yes") {
        Ok("arch".to_string())
    } else {
        Ok("unknown".to_string())
    }
}

/// Tauri system dependencies on Debian/Ubuntu that mise cannot install.
const TAURI_SYSTEM_DEPS_DEBIAN: &[&str] = &[
    "build-essential", "curl", "git", "pkg-config", "clang", "lld",
    "libwebkit2gtk-4.1-dev", "libgtk-3-dev", "libayatana-appindicator3-dev",
    "librsvg2-dev", "libssl-dev", "libxdo-dev", "libsoup-3.0-dev",
    "libjavascriptcoregtk-4.1-dev", "xvfb", "scrot", "openbox",
];

/// Tauri system dependencies on Arch Linux.
const TAURI_SYSTEM_DEPS_ARCH: &[&str] = &[
    "base-devel", "curl", "git", "pkg-config", "clang", "lld",
    "webkit2gtk", "gtk3", "libayatana-appindicator",
    "librsvg", "openssl", "libxdo",
    "libsoup3", "webkit2gtk-4.1",
    "xorg-server-xvfb", "scrot", "openbox",
];

pub fn bootstrap_linux(_profile: &VmProfile, session: &mut VmSession, logger: &BootstrapLogger) -> Result<()> {
    // Detect distro type
    let distro = detect_distro(session)?;
    logger.message(&format!("Detected distro: {}", distro));

    logger::step(logger, "install system deps", || {
        // Check if all critical deps are present, not just Xvfb
        let xvfb_check = crate::ssh::exec(
            session,
            "command -v Xvfb >/dev/null 2>&1 && echo present || echo missing",
        );
        let clang_check = crate::ssh::exec(
            session,
            "command -v clang >/dev/null 2>&1 && echo present || echo missing",
        );
        let xvfb_present = xvfb_check.unwrap_or_default();
        let clang_present = clang_check.unwrap_or_default();
        logger.message(&format!("  Xvfb check: {:?}", xvfb_present.trim()));
        logger.message(&format!("  Clang check: {:?}", clang_present.trim()));
        if !xvfb_present.contains("present") || !clang_present.contains("present") {
            match distro.as_str() {
                "debian" => {
                    let deps = TAURI_SYSTEM_DEPS_DEBIAN.join(" ");
                    logger.message(&format!("  Installing deps (Debian/Ubuntu): {}", deps));
                    let update_output = crate::ssh::exec(session, "sudo apt-get update -qq 2>&1")?;
                    logger.message(&format!("  apt-get update output: {}", update_output));
                    let install_cmd = format!("DEBIAN_FRONTEND=noninteractive sudo apt-get install -y {}", deps);
                    let install_output = crate::ssh::exec(session, &install_cmd)?;
                    logger.message(&format!("  apt-get install output: {}", install_output));
                }
                "arch" => {
                    let deps = TAURI_SYSTEM_DEPS_ARCH.join(" ");
                    logger.message(&format!("  Installing deps (Arch): {}", deps));
                    let update_output = crate::ssh::exec(session, "sudo pacman -Sy --noconfirm 2>&1")?;
                    logger.message(&format!("  pacman update output: {}", update_output));
                    let install_cmd = format!("sudo pacman -S --noconfirm {}", deps);
                    let install_output = crate::ssh::exec(session, &install_cmd)?;
                    logger.message(&format!("  pacman install output: {}", install_output));
                }
                _ => {
                    return Err(crate::config::TestbedError::BootstrapFailed {
                        step: "install system deps".to_string(),
                        message: format!("Unsupported distro: {}. Please install deps manually.", distro),
                    });
                }
            }
        } else {
            logger.message("  Skipping system deps install (all present)");
        }
        Ok(())
    })?;

    logger::step(logger, "install mise", || {
        let mise = crate::ssh::exec(session, "~/.local/bin/mise --version 2>/dev/null || echo missing")
            .unwrap_or_default();
        if !mise.contains("missing") && !mise.is_empty() {
            return Ok(());
        }
        // Run mise install directly
        let install_cmd = r#"curl -fsSL https://mise.run | sh && [ -f ~/.local/bin/mise ] && ~/.local/bin/mise --version"#;
        let install_output = crate::ssh::exec(session, install_cmd)?;
        logger.message(&format!("mise install output: {}", install_output));
        // Verify mise was actually installed
        let verify = crate::ssh::exec(
            session,
            "[ -f ~/.local/bin/mise ] && ~/.local/bin/mise --version || (echo 'mise install verification failed' && exit 1)",
        )?;
        if verify.contains("verification failed") {
            return Err(crate::config::TestbedError::BootstrapFailed {
                step: "install mise".to_string(),
                message: format!("mise binary not found after install. Output: {}", install_output),
            });
        }
        Ok(())
    })?;

    logger::step(logger, "activate mise in .bashrc", || {
        crate::ssh::exec(session, ACTIVATE_MISE_BASHRC_SH)?;
        Ok(())
    })?;

    logger::step(logger, "install cargo-binstall", || {
        let present = crate::ssh::exec(
            session,
            "[ -x \"$HOME/.cargo/bin/cargo-binstall\" ] && echo present || echo missing",
        )
        .unwrap_or_default();
        if present.contains("present") {
            return Ok(());
        }
        let arch = crate::ssh::exec(session, "uname -m").unwrap_or_default();
        let target = if arch.trim() == "aarch64" { "aarch64-unknown-linux-musl" } else { "x86_64-unknown-linux-musl" };
        let script = INSTALL_CARGO_BINSTALL_SH.replace("{{TARGET}}", &target);
        crate::ssh::exec(session, &script)?;
        Ok(())
    })?;

    logger::step(logger, "configure mise cargo_binstall", || {
        crate::ssh::exec(session, CONFIGURE_MISE_CARGO_BINSTALL_SH)?;
        Ok(())
    })?;

    logger::step(logger, "install tools via mise", || {
        let script = INSTALL_TOOLS_MISE_SH.replace("{{MISE_TOML}}", BOOTSTRAP_MISE_TOML);
        crate::ssh::exec(session, &script)?;
        crate::ssh::exec(session, "$HOME/.local/bin/mise exec -- rustc --version")?;
        Ok(())
    })?;

    logger::step(logger, "set nushell as default shell", || {
        crate::ssh::exec(session, SET_NUSHELL_DEFAULT_SHELL_SH)?;
        Ok(())
    })?;

    logger::step(logger, "authorise host SSH key", || {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));
        let key_names = ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"];
        let mut pub_key = String::new();
        for key_name in key_names {
            let path = home.join(".ssh").join(key_name);
            if path.exists() && let Ok(key) = std::fs::read_to_string(&path) {
                pub_key = key.trim().to_string();
                break;
            }
        }
        if !pub_key.is_empty() {
            let script = SETUP_SSH_KEYS_SH.replace("{{KEY}}", &pub_key);
            crate::ssh::exec(session, &script)?;
        }
        Ok(())
    })?;

    logger::step(logger, "set up project mount", || {
        setup_project_mount(session)
    })?;

    // Optional: Install GUI packages (not enabled by default)
    // This can be enabled via a profile flag or environment variable
    logger::step(logger, "install gui (optional)", || {
        if std::env::var("TESTBED_INSTALL_GUI").is_ok() {
            logger.message("  Installing GUI environment...");
            crate::ssh::exec(session, INSTALL_GUI_SH)?;
            logger.message("  GUI environment installed");
        } else {
            logger.message("  Skipping GUI install (set TESTBED_INSTALL_GUI=1 to enable)");
        }
        Ok(())
    })?;

    logger::step(logger, "write bootstrap marker", || {
        crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
        Ok(())
    })?;

    Ok(())
}

fn setup_project_mount(session: &mut VmSession) -> Result<()> {
    // Check if already mounted
    let check = crate::ssh::exec(
        session,
        "mount | grep -q '9p' && echo 'mounted' || echo 'not mounted'",
    )?;
    if check.contains("mounted") {
        return Ok(());
    }

    // Create mount point and mount
    let script = SETUP_PROJECT_MOUNT_SH;
    crate::ssh::exec(session, script)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tauri_deps_not_empty() {
        assert!(!TAURI_SYSTEM_DEPS.is_empty());
        assert!(TAURI_SYSTEM_DEPS.len() > 10);
    }

    #[test]
    fn test_bootstrap_mise_toml_has_required_tools() {
        let toml = crate::bootstrap::BOOTSTRAP_MISE_TOML;
        assert!(toml.contains("rust"));
        assert!(toml.contains("nu"));
        assert!(toml.contains("cargo:cargo-binstall"));
        assert!(toml.contains("cargo:sccache"));
        assert!(toml.contains("cargo:tauri-cli"));
    }
}
