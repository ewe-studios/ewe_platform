//! macOS bootstrap via SSH — step-by-step idempotent.

use crate::bootstrap::{BOOTSTRAP_MISE_TOML, logger};
use crate::bootstrap::BootstrapLogger;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;

pub fn bootstrap_macos(_profile: &VmProfile, session: &mut VmSession, logger: &BootstrapLogger) -> Result<()> {
    logger::step(logger, "enable remote login", || enable_remote_login(session))?;
    logger::step(logger, "authorise host SSH key", || setup_ssh_keys(session))?;
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
        if path.exists()
            && let Ok(key) = std::fs::read_to_string(&path) {
                pub_key = key.trim().to_string();
                break;
            }
    }
    if pub_key.is_empty() {
        return Ok(());
    }
    let key_escaped = shell_quote(&pub_key);
    let script = format!(
        r#"mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && \
           chmod 600 ~/.ssh/authorized_keys && \
           grep -qxF {key_escaped} ~/.ssh/authorized_keys || echo {key_escaped} >> ~/.ssh/authorized_keys"#
    );
    crate::ssh::exec(session, &script)?;
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
    crate::ssh::exec(session, "curl -fsSL https://mise.run | sh")?;
    Ok(())
}

fn activate_mise_in_zprofile(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(
        session,
        r#"grep -q 'mise activate' ~/.zprofile || echo 'eval "$($HOME/.local/bin/mise activate zsh)"' >> ~/.zprofile"#,
    )?;
    crate::ssh::exec(
        session,
        r#"grep -q 'mise activate' ~/.bashrc 2>/dev/null || echo 'eval "$($HOME/.local/bin/mise activate bash)"' >> ~/.bashrc 2>/dev/null || true"#,
    )?;
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
    crate::ssh::exec(
        session,
        &format!(
            "mkdir -p ~/.cargo/bin && \
             curl -sSfL https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-{target}.tar.gz | tar -xz -C ~/.cargo/bin && \
             chmod +x ~/.cargo/bin/cargo-binstall"
        ),
    )?;
    Ok(())
}

fn configure_mise_cargo_binstall(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(
        session,
        "mkdir -p ~/.config/mise && \
         touch ~/.config/mise/config.toml && \
         (grep -q 'cargo_binstall' ~/.config/mise/config.toml || \
          printf '\\n[settings]\\ncargo_binstall = true\\n' >> ~/.config/mise/config.toml)",
    )?;
    Ok(())
}

fn install_tools(session: &mut VmSession) -> Result<()> {
    let script = format!(
        r#"cat <<'MISE_EOF' > /tmp/bootstrap-mise.toml
{mise_toml_content}
MISE_EOF
export MISE_CONFIG_FILE=/tmp/bootstrap-mise.toml
$HOME/.local/bin/mise install
rm -f /tmp/bootstrap-mise.toml"#,
        mise_toml_content = BOOTSTRAP_MISE_TOML,
    );
    crate::ssh::exec(session, &script)?;
    crate::ssh::exec(session, "$HOME/.local/bin/mise exec -- rustc --version")?;
    Ok(())
}

fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(
        session,
        r#"NU_PATH=$(find ~/.local/share/mise/installs/nu -name nu -type f 2>/dev/null | head -1); if [ -n "$NU_PATH" ]; then chsh -s "$NU_PATH" 2>/dev/null || true; fi"#,
    )?;
    Ok(())
}

fn write_bootstrap_marker(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
    Ok(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
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

    #[test]
    fn test_shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("hello"), "'hello'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
