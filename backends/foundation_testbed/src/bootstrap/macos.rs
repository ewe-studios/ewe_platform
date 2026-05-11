//! macOS bootstrap via SSH — step-by-step idempotent.

use crate::bootstrap::{BOOTSTRAP_MISE_TOML, logger};
use crate::bootstrap::BootstrapLogger;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;

const ACTIVATE_MISE_ZPROFILE_SH: &str = include_str!("../../scripts/macos/activate_mise_zprofile.sh");
const INSTALL_MISE_SH: &str = include_str!("../../scripts/macos/install_mise.sh");
const INSTALL_CARGO_BINSTALL_SH: &str = include_str!("../../scripts/macos/install_cargo_binstall.sh");
const CONFIGURE_MISE_CARGO_BINSTALL_SH: &str = include_str!("../../scripts/macos/configure_mise_cargo_binstall.sh");
const INSTALL_TOOLS_MISE_SH: &str = include_str!("../../scripts/macos/install_tools_mise.sh");
const SET_NUSHELL_DEFAULT_SHELL_SH: &str = include_str!("../../scripts/macos/set_nushell_default_shell.sh");
const SETUP_SSH_KEYS_SH: &str = include_str!("../../scripts/macos/setup_ssh_keys.sh");
const SETUP_PROJECT_MOUNT_SH: &str = include_str!("../../scripts/macos/setup_project_mount.sh");

pub fn bootstrap_macos(_profile: &VmProfile, session: &mut VmSession, logger: &BootstrapLogger) -> Result<()> {
    logger::step(logger, "enable remote login", || enable_remote_login(session))?;
    logger::step(logger, "authorise host SSH key", || setup_ssh_keys(session))?;
    logger::step(logger, "set up project mount", || setup_project_mount(session))?;
    logger::step(logger, "install Xcode CLT", || install_xcode_clt(session))?;
    logger::step(logger, "install mise", || install_mise(session))?;
    logger::step(logger, "activate mise in .zprofile", || activate_mise_in_zprofile(session))?;
    logger::step(logger, "install cargo-binstall", || install_cargo_binstall(session))?;
    logger::step(logger, "configure mise cargo_binstall", || configure_mise_cargo_binstall(session))?;
    logger::step(logger, "install tools via mise", || install_tools(session))?;
    logger::step(logger, "set nushell as default shell", || set_nushell_default_shell(session))?;
    logger::step(logger, "write bootstrap marker", || write_bootstrap_marker(session))?;
    Ok(())
}

fn enable_remote_login(session: &mut VmSession) -> Result<()> {
    let check = crate::ssh::exec(session, "systemsetup -getremotelogin 2>/dev/null")?;
    if check.to_lowercase().contains("on") {
        return Ok(());
    }
    crate::ssh::exec(session, "sudo systemsetup -setremotelogin on")?;
    Ok(())
}

fn setup_ssh_keys(session: &mut VmSession) -> Result<()> {
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
    if pub_key.is_empty() {
        return Ok(());
    }
    let script = SETUP_SSH_KEYS_SH.replace("{{KEY}}", &pub_key);
    crate::ssh::exec(session, &script)?;
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

fn install_xcode_clt(session: &mut VmSession) -> Result<()> {
    let clang_check = crate::ssh::exec(session, "command -v clang >/dev/null 2>&1 && echo present || echo missing")?;
    if clang_check.contains("present") {
        let sdk_check = crate::ssh::exec(session, "xcrun --show-sdk-path 2>/dev/null && echo present || echo missing")?;
        if sdk_check.contains("present") {
            return Ok(());
        }
    }
    let output = crate::ssh::exec(
        session,
        "softwareupdate --list 2>&1 | grep -i 'Command Line Tools' | head -1",
    )?;
    if output.trim().is_empty() {
        crate::ssh::exec(session, "xcode-select --install 2>&1")?;
        return Err(crate::config::TestbedError::BootstrapFailed {
            step: "xcode_clt".to_string(),
            message: "Xcode CLT not found. Run 'xcode-select --install' manually in the VM, then re-run bootstrap.".to_string(),
        });
    }
    crate::ssh::exec(session, "softwareupdate --all --install --agree-to-license 2>&1")?;
    Ok(())
}

fn install_mise(session: &mut VmSession) -> Result<()> {
    let mise = crate::ssh::exec(session, "~/.local/bin/mise --version 2>/dev/null || echo missing")?;
    if !mise.contains("missing") && !mise.is_empty() {
        return Ok(());
    }
    crate::ssh::exec(session, INSTALL_MISE_SH)?;
    Ok(())
}

fn activate_mise_in_zprofile(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, ACTIVATE_MISE_ZPROFILE_SH)?;
    Ok(())
}

fn install_cargo_binstall(session: &mut VmSession) -> Result<()> {
    let present = crate::ssh::exec(
        session,
        "[ -x \"$HOME/.cargo/bin/cargo-binstall\" ] && echo present || echo missing",
    )?;
    if present.contains("present") {
        return Ok(());
    }
    let arch = crate::ssh::exec(session, "uname -m").unwrap_or_default();
    let target = if arch.trim() == "arm64" { "aarch64-apple-darwin" } else { "x86_64-apple-darwin" };
    let script = INSTALL_CARGO_BINSTALL_SH.replace("{{TARGET}}", &target);
    crate::ssh::exec(session, &script)?;
    Ok(())
}

fn configure_mise_cargo_binstall(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, CONFIGURE_MISE_CARGO_BINSTALL_SH)?;
    Ok(())
}

fn install_tools(session: &mut VmSession) -> Result<()> {
    let script = INSTALL_TOOLS_MISE_SH.replace("{{MISE_TOML}}", BOOTSTRAP_MISE_TOML);
    crate::ssh::exec(session, &script)?;
    crate::ssh::exec(session, "$HOME/.local/bin/mise exec -- rustc --version")?;
    Ok(())
}

fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, SET_NUSHELL_DEFAULT_SHELL_SH)?;
    Ok(())
}

fn write_bootstrap_marker(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
