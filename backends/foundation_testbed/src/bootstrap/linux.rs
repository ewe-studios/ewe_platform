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

/// Tauri system dependencies on Debian/Ubuntu that mise cannot install.
const TAURI_SYSTEM_DEPS: &[&str] = &[
    "build-essential", "curl", "git", "pkg-config",
    "libwebkit2gtk-4.1-dev", "libgtk-3-dev", "libayatana-appindicator3-dev",
    "librsvg2-dev", "libssl-dev", "libxdo-dev", "libsoup-3.0-dev",
    "libjavascriptcoregtk-4.1-dev", "xvfb", "scrot", "openbox",
];

pub fn bootstrap_linux(_profile: &VmProfile, session: &mut VmSession, logger: &BootstrapLogger) -> Result<()> {
    logger::step(logger, "install system deps", || {
        let xvfb_present = crate::ssh::exec(
            session,
            "command -v Xvfb >/dev/null 2>&1 && echo present || echo missing",
        )
        .unwrap_or_default();
        if !xvfb_present.contains("present") {
            let deps = TAURI_SYSTEM_DEPS.join(" ");
            let script = INSTALL_SYSTEM_DEPS_SH.replace("{{DEPS}}", &deps);
            crate::ssh::exec(session, &script)?;
        }
        Ok(())
    })?;

    logger::step(logger, "install mise", || {
        let mise = crate::ssh::exec(session, "~/.local/bin/mise --version 2>/dev/null || echo missing")
            .unwrap_or_default();
        if !mise.contains("missing") && !mise.is_empty() {
            return Ok(());
        }
        crate::ssh::exec(session, INSTALL_MISE_SH)?;
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

    logger::step(logger, "write bootstrap marker", || {
        crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
        Ok(())
    })?;

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
