//! Cargo build orchestrator — runs `cargo build --target wasm32-unknown-unknown`.
//!
//! WHY: We need to compile the user's crate to wasm32-unknown-unknown before testing.
//! WHAT: Invokes cargo as a subprocess, captures output, locates the resulting .wasm.
//! HOW: Uses std::process::Command with inherited stdout/stderr so the user
//!      sees build output in real time.

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

/// Output of a successful wasm build.
pub struct BuildOutput {
    /// Absolute path to the built .wasm file.
    pub wasm_path: PathBuf,
    /// Cargo package name.
    pub package_name: String,
    /// Build profile: "debug" or "release".
    pub profile: String,
}

/// Read the package name from a Cargo.toml file.
fn read_package_name(cargo_toml: &Path) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(cargo_toml)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {e}", cargo_toml.display()))?;

    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') && in_package {
            in_package = false;
            continue;
        }
        if in_package && trimmed.starts_with("name") {
            if let Some(value) = trimmed.split('=').nth(1) {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                return Ok(value.to_string());
            }
        }
    }

    anyhow::bail!("Cargo.toml missing [package].name in {}", cargo_toml.display())
}

/// Run `cargo build --target wasm32-unknown-unknown` and return the wasm output.
///
/// # Errors
///
/// Returns an error if:
/// - cargo is not on PATH
/// - the build fails
/// - the wasm binary is not found after a successful build
/// - the crate has no [package].name
pub fn run(
    crate_path: &Path,
    release: bool,
    features: Option<&str>,
) -> anyhow::Result<BuildOutput> {
    let cargo_toml = crate_path.join("Cargo.toml");
    let package_name = read_package_name(&cargo_toml)?;

    let profile = if release { "release" } else { "debug" };

    // Check that cargo is available
    which::which("cargo").map_err(|_| {
        anyhow::anyhow!("cargo not found on PATH")
    })?;

    info!("Running cargo build --target wasm32-unknown-unknown...");

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .current_dir(crate_path);

    if release {
        cmd.arg("--release");
    }

    if let Some(features) = features {
        cmd.arg("--features").arg(features);
    }

    debug!("Executing: {:?}", cmd);

    let status = cmd
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("cargo not found on PATH")
            } else {
                anyhow::anyhow!("Failed to execute cargo: {e}")
            }
        })?;

    if !status.success() {
        anyhow::bail!(
            "cargo build failed (exit code {:?})\n\
            Hint: if wasm32-unknown-unknown target is not installed, run:\n\
            rustup target add wasm32-unknown-unknown",
            status.code()
        );
    }

    let wasm_path = crate_path
        .join("target")
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(format!("{package_name}.wasm"));

    if !wasm_path.exists() {
        anyhow::bail!(
            "Wasm binary not found at {}\n\
            Expected cargo build to produce this file.",
            wasm_path.display()
        );
    }

    info!("Built wasm: {}", wasm_path.display());

    Ok(BuildOutput {
        wasm_path,
        package_name,
        profile: profile.to_string(),
    })
}
