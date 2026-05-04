//! Linux bootstrap via SSH — step-by-step idempotent.
//!
//! Each step checks concrete artifacts before running. If bootstrap fails
//! mid-way, re-running picks up where it left off.

use std::time::Instant;

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
    step("install system deps", || {
        let xvfb_present = crate::ssh::exec(
            session,
            "command -v Xvfb >/dev/null 2>&1 && echo present || echo missing",
        )
        .unwrap_or_default();
        if !xvfb_present.contains("present") {
            let deps = TAURI_SYSTEM_DEPS.join(" ");
            crate::ssh::exec(session, "apt-get update -qq")?;
            crate::ssh::exec(
                session,
                &format!("DEBIAN_FRONTEND=noninteractive apt-get install -y {deps}"),
            )?;
        }
        Ok(())
    })?;

    step("install mise", || {
        let mise = crate::ssh::exec(
            session,
            "~/.local/bin/mise --version 2>/dev/null || echo missing",
        )
        .unwrap_or_default();
        if !mise.contains("missing") && !mise.is_empty() {
            return Ok(());
        }
        crate::ssh::exec(session, "curl -fsSL https://mise.run | sh")?;
        Ok(())
    })?;

    step("activate mise in .bashrc", || {
        crate::ssh::exec(
            session,
            r#"grep -q 'mise activate' ~/.bashrc || echo 'eval "$($HOME/.local/bin/mise activate bash)"' >> ~/.bashrc"#,
        )?;
        Ok(())
    })?;

    step("install cargo-binstall", || {
        let present = crate::ssh::exec(
            session,
            "[ -x \"$HOME/.cargo/bin/cargo-binstall\" ] && echo present || echo missing",
        )
        .unwrap_or_default();
        if present.contains("present") {
            return Ok(());
        }
        let arch = crate::ssh::exec(session, "uname -m").unwrap_or_default();
        let target = if arch.trim() == "aarch64" {
            "aarch64-unknown-linux-musl"
        } else {
            "x86_64-unknown-linux-musl"
        };
        crate::ssh::exec(
            session,
            &format!(
                "mkdir -p ~/.cargo/bin && \
                 curl -sSfL https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-{target}.tgz | tar -xz -C ~/.cargo/bin && \
                 chmod +x ~/.cargo/bin/cargo-binstall"
            ),
        )?;
        Ok(())
    })?;

    step("configure mise cargo_binstall", || {
        crate::ssh::exec(
            session,
            "mkdir -p ~/.config/mise && \
             touch ~/.config/mise/config.toml && \
             (grep -q 'cargo_binstall' ~/.config/mise/config.toml || \
              printf '\\n[settings]\\ncargo_binstall = true\\n' >> ~/.config/mise/config.toml)",
        )?;
        Ok(())
    })?;

    step("install tools via mise", || {
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
    })?;

    step("set nushell as default shell", || {
        crate::ssh::exec(
            session,
            r#"NU_PATH=$(find ~/.local/share/mise/installs/nu -name nu -type f 2>/dev/null | head -1); if [ -n "$NU_PATH" ]; then chsh -s "$NU_PATH" 2>/dev/null || true; fi"#,
        )?;
        Ok(())
    })?;

    step("authorise host SSH key", || {
        // Set up host SSH public key in authorized_keys
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
        if !pub_key.is_empty() {
            let key_escaped = pub_key.replace('\'', r"'\''");
            let script = format!(
                r#"mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && \
                   chmod 600 ~/.ssh/authorized_keys && \
                   grep -qxF '{key_escaped}' ~/.ssh/authorized_keys || echo '{key_escaped}' >> ~/.ssh/authorized_keys"#
            );
            crate::ssh::exec(session, &script)?;
        }
        Ok(())
    })?;

    step("write bootstrap marker", || {
        crate::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
        Ok(())
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
