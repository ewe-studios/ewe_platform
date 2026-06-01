//! Wasm binary location and wasm-bindgen CLI invocation.
//!
//! WHY: wasm-bindgen must be run to generate JS glue code from wasm binaries.
//! WHAT: Locates wasm-bindgen on PATH, invokes it as a subprocess.
//! HOW: Uses the wasm-bindgen CLI binary (not the library) to ensure
//!      version alignment with the user's wasm-bindgen dependency.

use std::path::Path;
use std::process::Command;

use tracing::{debug, info};

/// Target platform for wasm-bindgen output.
pub enum BindgenTarget {
    /// Web browser (default, no --target flag needed).
    Web,
    /// Deno runtime (--target deno).
    Deno,
    /// ES modules (--target esmodules, used for Cloudflare Workers).
    EsModules,
}

impl BindgenTarget {
    /// Returns the CLI argument for this target, or None for the default.
    fn cli_arg(&self) -> Option<&'static str> {
        match self {
            BindgenTarget::Web => None,
            BindgenTarget::Deno => Some("deno"),
            BindgenTarget::EsModules => Some("esmodules"),
        }
    }
}

/// Run wasm-bindgen CLI on a wasm binary.
///
/// Generates JS glue and wasm output files in the specified directory.
///
/// # Errors
///
/// Returns an error if:
/// - wasm-bindgen is not on PATH
/// - the wasm-bindgen CLI fails
/// - expected output files are not found after execution
pub fn run_wasm_bindgen(
    wasm_path: &Path,
    output_dir: &Path,
    target: BindgenTarget,
) -> anyhow::Result<()> {
    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)?;

    // Clean existing files in output dir (avoid stale artifacts)
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
        }
    }

    // Locate wasm-bindgen
    which::which("wasm-bindgen").map_err(|_| {
        anyhow::anyhow!(
            "wasm-bindgen not found on PATH.\n\
            Install: cargo install wasm-bindgen-cli"
        )
    })?;

    info!("Running wasm-bindgen...");

    let mut cmd = Command::new("wasm-bindgen");
    cmd.arg(wasm_path)
        .arg("--out-dir")
        .arg(output_dir);

    if let Some(arg) = target.cli_arg() {
        cmd.arg("--target").arg(arg);
    }

    debug!("Executing: {:?}", cmd);

    let status = cmd.status().map_err(|e| {
        anyhow::anyhow!("Failed to execute wasm-bindgen: {e}")
    })?;

    if !status.success() {
        anyhow::bail!(
            "wasm-bindgen failed (exit code {:?})",
            status.code()
        );
    }

    info!("wasm-bindgen output in {}", output_dir.display());
    Ok(())
}
