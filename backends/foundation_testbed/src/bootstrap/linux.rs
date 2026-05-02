//! Linux bootstrap via SSH.
//!
//! Steps: base packages → Tauri deps → mise → tools → nushell → SSH keys → marker.

use crate::bootstrap::BOOTSTRAP_MISE_TOML;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;

/// Tauri system dependencies on Debian/Ubuntu that mise cannot install.
const TAURI_SYSTEM_DEPS: &[&str] = &[
    "build-essential",
    "curl",
    "git",
    "pkg-config",
    "libwebkit2gtk-4.1-dev",
    "libgtk-3-dev",
    "libayatana-appindicator3-dev",
    "librsvg2-dev",
    "libssl-dev",
    "libxdo-dev",
    "libsoup-3.0-dev",
    "libjavascriptcoregtk-4.1-dev",
    "xvfb",
    "scrot",
    "openbox",
];

/// Bootstrap a Linux VM with development tools.
pub fn bootstrap_linux(_profile: &VmProfile, session: &mut VmSession) -> Result<()> {
    // Step 1: Install base packages (only things mise can't provide)
    install_system_deps(session)?;

    // Step 2: Install mise via official installer
    install_mise(session)?;

    // Step 3: Write bootstrap mise.toml and run mise install
    install_tools(session)?;

    // Step 4: Set nushell as default shell
    set_nushell_default_shell(session)?;

    // Step 5: Set up host SSH key authorization
    setup_ssh_keys(session)?;

    // Step 6: Write bootstrap marker
    write_bootstrap_marker(session)?;

    Ok(())
}

/// Install system dependencies via apt.
fn install_system_deps(session: &mut VmSession) -> Result<()> {
    let deps = TAURI_SYSTEM_DEPS.join(" ");
    crate::ssh::exec(session, &format!("apt-get update -qq"))?;
    crate::ssh::exec(session, &format!("DEBIAN_FRONTEND=noninteractive apt-get install -y {deps}"))?;
    Ok(())
}

/// Install mise on Linux via official installer.
fn install_mise(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, "curl -fsSL https://mise.run | sh")?;
    // Add mise to PATH
    crate::ssh::exec(session, r#"echo 'eval "$($HOME/.local/bin/mise activate bash)"' >> ~/.bashrc"#)?;
    Ok(())
}

/// Install development tools via bootstrap mise.toml.
fn install_tools(session: &mut VmSession) -> Result<()> {
    // Write bootstrap mise.toml
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

    // Verify key tools are installed
    crate::ssh::exec(session, "$HOME/.local/bin/mise exec -- rustc --version")?;

    Ok(())
}

/// Set nushell as the default shell.
fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    // Find nu binary path and set as default shell
    crate::ssh::exec(session, r#"NU_PATH=$(find ~/.local/share/mise/installs/nu -name nu -type f 2>/dev/null | head -1); if [ -n "$NU_PATH" ]; then chsh -s "$NU_PATH" 2>/dev/null || true; fi"#)?;
    Ok(())
}

/// Set up host SSH public key in authorized_keys.
fn setup_ssh_keys(session: &mut VmSession) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));

    // Try ed25519 first, then rsa, then ecdsa
    let key_names = ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"];
    let mut pub_key = String::new();

    for key_name in key_names {
        let path = home.join(".ssh").join(key_name);
        if path.exists() {
            if let Ok(key) = std::fs::read_to_string(&path) {
                pub_key = key;
                break;
            }
        }
    }

    if !pub_key.is_empty() {
        let key_escaped = pub_key.replace("'", "'\\''");
        let script = format!(
            r#"mkdir -p ~/.ssh
chmod 700 ~/.ssh
touch ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys
KEY='{key_escaped}'
if ! grep -qF "$KEY" ~/.ssh/authorized_keys; then
    echo "$KEY" >> ~/.ssh/authorized_keys
fi"#
        );
        crate::ssh::exec(session, &script)?;
    }

    Ok(())
}

/// Write the bootstrap completion marker.
fn write_bootstrap_marker(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
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
