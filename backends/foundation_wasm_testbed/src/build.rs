//! Cargo build orchestrator — runs `cargo build --target wasm32-unknown-unknown`.
//!
//! WHY: We need to compile the user's crate to wasm32-unknown-unknown before testing.
//! WHAT: Invokes cargo as a subprocess, captures output, locates the resulting .wasm.
//! HOW: Uses `std::process::Command` with inherited stdout/stderr so the user
//!      sees build output in real time.

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use crate::error::{Result, ToTrace, WasmTestbedError};

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
fn read_package_name(cargo_toml: &Path) -> Result<String> {
    let content = std::fs::read_to_string(cargo_toml)
        .map_err(|_e| WasmTestbedError::MissingPackageName(cargo_toml.display().to_string()).trace())?;

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

    Err(WasmTestbedError::MissingPackageName(cargo_toml.display().to_string()).trace())
}

/// Find the workspace root Cargo.toml by walking up from `crate_path`.
/// Returns the workspace root if the crate is part of a workspace,
/// or None if it's a standalone crate.
fn find_workspace_root(crate_path: &Path) -> Option<PathBuf> {
    let mut current = crate_path.canonicalize().ok()?;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml).ok()?;
            if content.contains("[workspace]") {
                return Some(current.clone());
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Run `cargo build --target wasm32-unknown-unknown` and return the wasm output.
///
/// Expects the crate to have `crate-type = ["cdylib"]` in its `[lib]` section
/// to produce a `.wasm` binary. Detects workspace membership to locate the
/// correct target directory.
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
) -> Result<BuildOutput> {
    run_impl(crate_path, release, features, false)
}

/// Run `cargo build --target wasm32-unknown-unknown --tests` and return the wasm output.
///
/// Includes dev-dependencies so that `#[wasm_bindgen_test]` macros are expanded
/// and `__wbgt_` test exports are present in the wasm binary.
///
/// # Errors
/// Returns an error when cargo fails, the package name can't be read, or the
/// built wasm artifact can't be located.
pub fn run_with_tests(
    crate_path: &Path,
    release: bool,
    features: Option<&str>,
) -> Result<BuildOutput> {
    run_impl(crate_path, release, features, true)
}

fn run_impl(
    crate_path: &Path,
    release: bool,
    features: Option<&str>,
    include_tests: bool,
) -> Result<BuildOutput> {
    let cargo_toml = crate_path.join("Cargo.toml");
    let package_name = read_package_name(&cargo_toml)?;

    let profile = if release { "release" } else { "debug" };

    which::which("cargo").map_err(|_| WasmTestbedError::CargoNotFound.trace())?;

    info!("Running cargo build --target wasm32-unknown-unknown...");

    // Determine target directory: workspace members use workspace root's target.
    let target_dir = find_workspace_root(crate_path)
        .map_or_else(|| crate_path.join("target"), |root| root.join("target"));

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--target-dir")
        .arg(&target_dir);

    if include_tests {
        cmd.arg("--tests");
    }

    // Override dev profile: use LLVM backend (Cranelift doesn't support wasm32).
    cmd.env("CARGO_PROFILE_DEV_CODEGEN_BACKEND", "llvm")
        .current_dir(crate_path);

    if release {
        cmd.arg("--release");
    }

    if let Some(features) = features {
        cmd.arg("--features").arg(features);
    }

    debug!("Executing: {:?}", cmd);

    let status = cmd.status().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            WasmTestbedError::CargoNotFound.trace()
        } else {
            WasmTestbedError::CargoExecFailed(e).trace()
        }
    })?;

    if !status.success() {
        return Err(WasmTestbedError::CargoBuildFailed(status.code()).trace());
    }

    let deps_dir = target_dir
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join("deps");

    // When building with --tests, we need the test integration wasm binary
    // which contains the __wbgt_ exports. Look for mod-*.wasm in deps/.
    if include_tests && deps_dir.exists() {
        // Find the test wasm binary (mod-*.wasm or package_name_test-*.wasm)
        for entry in std::fs::read_dir(&deps_dir).ok().into_iter().flatten().flatten() {
            {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                // Test binaries start with "mod-" or "{package_name}-" with a hash
                if (name.starts_with("mod-") || name.starts_with(&format!("{package_name}-")))
                    && name.ends_with(".wasm")
                    && name != format!("{package_name}.wasm")
                {
                    let path = entry.path();
                    // Verify it has __wbgt_ exports
                    if let Ok(content) = std::fs::read(&path) {
                        let needle = b"__wbgt_";
                        if content.windows(7).any(|w| w == needle) {
                            info!("Built test wasm: {}", path.display());
                            return Ok(BuildOutput {
                                wasm_path: path,
                                package_name,
                                profile: profile.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Try top-level path first, then deps/ fallback for library wasm
    let wasm_path = target_dir
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(format!("{package_name}.wasm"));

    if !wasm_path.exists() {
        let deps_wasm = deps_dir.join(format!("{package_name}.wasm"));
        if deps_wasm.exists() {
            info!("Built wasm (deps/): {}", deps_wasm.display());
            return Ok(BuildOutput {
                wasm_path: deps_wasm,
                package_name,
                profile: profile.to_string(),
            });
        }
        return Err(WasmTestbedError::WasmBinaryNotFound(wasm_path.display().to_string()).trace());
    }
    info!("Built wasm: {}", wasm_path.display());

    Ok(BuildOutput {
        wasm_path,
        package_name,
        profile: profile.to_string(),
    })
}
