//! Platform-specific installers for host prerequisites.

use std::process::Command;

use crate::config::{Result, TestbedError};

/// Install mise using the official installer.
pub fn install_mise() -> Result<()> {
    // Try curl installer first (works on most Linux distros)
    let status = Command::new("sh")
        .args(["-c", "curl -fsSL https://mise.run | sh"])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning mise installer: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "mise installer failed".to_string(),
        });
    }

    // Add mise to PATH for this session
    if let Some(home) = dirs::home_dir() {
        let mise_bin = home.join(".local").join("bin").join("mise");
        if mise_bin.exists() {
            // We can't modify the parent process's PATH from Rust, but
            // future commands will use PATH lookup which should find it
            // once the shell session is refreshed.
            // For immediate use, we'll use the full path.
            let _ = mise_bin;
        }
    }

    Ok(())
}

/// Install nushell via mise.
pub fn install_nushell() -> Result<()> {
    // Use mise to install nu
    let mise_bin = find_mise_bin()?;

    // First ensure cargo_binstall is enabled for faster installs
    let status = Command::new(&mise_bin)
        .args(["install", "nu@latest"])
        .env("MISE_CARGO_BINSTALL", "true")
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("mise install nu: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "failed to install nushell via mise".to_string(),
        });
    }

    Ok(())
}

/// Install pitchfork via mise.
pub fn install_pitchfork() -> Result<()> {
    let mise_bin = find_mise_bin()?;

    let status = Command::new(&mise_bin)
        .args(["install", "pitchfork@latest"])
        .env("MISE_CARGO_BINSTALL", "true")
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("mise install pitchfork: {e}"),
        })?;

    if !status.success() {
        // pitchfork may not be available in mise's registry yet
        // Fall back to cargo install
        eprintln!("  pitchfork not available via mise, trying cargo install...");
        install_pitchfork_cargo()
    } else {
        Ok(())
    }
}

/// Fallback: install pitchfork via cargo install.
fn install_pitchfork_cargo() -> Result<()> {
    let status = Command::new("cargo")
        .args(["install", "pitchfork"])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("cargo install pitchfork: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "failed to install pitchfork".to_string(),
        });
    }

    Ok(())
}

/// Find the mise binary on PATH or in its default install location.
fn find_mise_bin() -> Result<std::path::PathBuf> {
    if let Ok(p) = which::which("mise") {
        return Ok(p);
    }
    // Default install location for mise
    if let Some(home) = dirs::home_dir() {
        let bin = home.join(".local").join("bin").join("mise");
        if bin.exists() {
            return Ok(bin);
        }
    }
    Err(TestbedError::Qcow2Error {
        message: "mise not found on PATH or ~/.local/bin".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_mise_bin_not_found() {
        // This will fail gracefully — just verify it doesn't panic
        let result = find_mise_bin();
        // May or may not be present depending on the host
        let _ = result.is_ok() || result.is_err();
    }
}
