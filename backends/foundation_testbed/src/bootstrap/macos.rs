//! macOS bootstrap via SSH — step-by-step idempotent.
//!
//! Each step checks concrete artifacts before running. If bootstrap fails
//! mid-way, re-running picks up where it left off.
//!
//! macOS differs from Linux in:
//! - No apt-get / package manager for system deps (relies on Xcode CLT)
//! - Xcode CLT required for C compiler, SDK headers, git, make
//! - mise installed to ~/.local/bin via same curl installer
//! - No nushell chsh (uses different default shell mechanism)

use std::time::Instant;

use crate::bootstrap::BOOTSTRAP_MISE_TOML;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;

/// Bootstrap a macOS VM with development tools.
pub fn bootstrap_macos(_profile: &VmProfile, session: &mut VmSession) -> Result<()> {
    // Step 1: Enable SSH remote login (idempotent)
    step("enable remote login", || {
        enable_remote_login(session)
    })?;

    // Step 2: SSH key authorization
    step("authorise host SSH key", || {
        setup_ssh_keys(session)
    })?;

    // Step 3: Xcode Command Line Tools
    step("install Xcode CLT", || {
        install_xcode_clt(session)
    })?;

    // Step 4: Install mise
    step("install mise", || {
        install_mise(session)
    })?;

    // Step 5: Activate mise in shell
    step("activate mise in .zprofile", || {
        activate_mise_in_zprofile(session)
    })?;

    // Step 6: cargo-binstall
    step("install cargo-binstall", || {
        install_cargo_binstall(session)
    })?;

    // Step 7: Configure mise cargo_binstall setting
    step("configure mise cargo_binstall", || {
        configure_mise_cargo_binstall(session)
    })?;

    // Step 8: Install tools via bootstrap mise.toml
    step("install tools via mise", || {
        install_tools(session)
    })?;

    // Step 9: Set nushell as default shell
    step("set nushell as default shell", || {
        set_nushell_default_shell(session)
    })?;

    // Step 10: Write bootstrap marker
    step("write bootstrap marker", || {
        write_bootstrap_marker(session)
    })?;

    Ok(())
}

/// Run a named bootstrap step, tracking elapsed time.
fn step<F>(label: &str, f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let start = Instant::now();
    f()?;
    let elapsed = start.elapsed();
    let _ = (label, elapsed);
    Ok(())
}

/// Enable SSH remote login (idempotent).
fn enable_remote_login(session: &mut VmSession) -> Result<()> {
    let check = crate::ssh::exec(session, "systemsetup -getremotelogin 2>/dev/null")?;
    if check.to_lowercase().contains("on") {
        return Ok(());
    }
    crate::ssh::exec(session, "sudo systemsetup -setremotelogin on")?;
    Ok(())
}

/// Set up host SSH public key in authorized_keys.
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

/// Install Xcode Command Line Tools (idempotent).
///
/// Checks for clang and SDK headers before triggering install.
/// On macOS, xcode-select --install opens a GUI prompt. In headless
/// mode, we use the softwareupdate method which can be scripted.
fn install_xcode_clt(session: &mut VmSession) -> Result<()> {
    // Check if clang is available and has SDK headers
    let clang_check = crate::ssh::exec(session, "command -v clang >/dev/null 2>&1 && echo present || echo missing")?;
    if clang_check.contains("present") {
        // Further check: do we have SDK headers?
        let sdk_check = crate::ssh::exec(session, "xcrun --show-sdk-path 2>/dev/null && echo present || echo missing")?;
        if sdk_check.contains("present") {
            return Ok(());
        }
    }

    // Try softwareupdate method (works headless on newer macOS)
    let output = crate::ssh::exec(
        session,
        "softwareupdate --list 2>&1 | grep -i 'Command Line Tools' | head -1",
    )?;

    if output.trim().is_empty() {
        // Already installed or not available via softwareupdate
        // Try xcode-select --install as fallback (opens GUI prompt)
        crate::ssh::exec(session, "xcode-select --install 2>&1")?;
        return Err(crate::config::TestbedError::BootstrapFailed {
            step: "xcode_clt".to_string(),
            message: "Xcode CLT not found. Run 'xcode-select --install' manually in the VM, then re-run bootstrap.".to_string(),
        });
    }

    // Install via softwareupdate
    crate::ssh::exec(
        session,
        "softwareupdate --all --install --agree-to-license 2>&1",
    )?;
    Ok(())
}

/// Install mise version manager.
fn install_mise(session: &mut VmSession) -> Result<()> {
    let mise = crate::ssh::exec(
        session,
        "~/.local/bin/mise --version 2>/dev/null || echo missing",
    )?;
    if !mise.contains("missing") && !mise.is_empty() {
        return Ok(());
    }
    crate::ssh::exec(session, "curl -fsSL https://mise.run | sh")?;
    Ok(())
}

/// Activate mise in .zprofile (macOS default shell is zsh).
fn activate_mise_in_zprofile(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(
        session,
        r#"grep -q 'mise activate' ~/.zprofile || echo 'eval "$($HOME/.local/bin/mise activate zsh)"' >> ~/.zprofile"#,
    )?;
    // Also add to .bashrc for SSH sessions that use bash
    crate::ssh::exec(
        session,
        r#"grep -q 'mise activate' ~/.bashrc 2>/dev/null || echo 'eval "$($HOME/.local/bin/mise activate bash)"' >> ~/.bashrc 2>/dev/null || true"#,
    )?;
    Ok(())
}

/// Install cargo-binstall binary downloader.
fn install_cargo_binstall(session: &mut VmSession) -> Result<()> {
    let present = crate::ssh::exec(
        session,
        "[ -x \"$HOME/.cargo/bin/cargo-binstall\" ] && echo present || echo missing",
    )?;
    if present.contains("present") {
        return Ok(());
    }
    // macOS on Intel = x86_64-apple-darwin, Apple Silicon = aarch64-apple-darwin
    let arch = crate::ssh::exec(session, "uname -m").unwrap_or_default();
    let target = if arch.trim() == "arm64" {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
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

/// Configure mise to use cargo-binstall.
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

/// Install tools via bootstrap mise.toml.
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

/// Set nushell as the default shell via chsh.
fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(
        session,
        r#"NU_PATH=$(find ~/.local/share/mise/installs/nu -name nu -type f 2>/dev/null | head -1); if [ -n "$NU_PATH" ]; then chsh -s "$NU_PATH" 2>/dev/null || true; fi"#,
    )?;
    Ok(())
}

/// Write the bootstrap completion marker.
fn write_bootstrap_marker(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
    Ok(())
}

/// Quote a string for safe passing to a shell command.
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
