//! Wasm binary location and wasm-bindgen CLI invocation.
//!
//! WHY: wasm-bindgen must be run to generate JS glue code from wasm binaries.
//! WHAT: Locates wasm-bindgen on PATH, invokes it as a subprocess.
//! HOW: Uses the wasm-bindgen CLI binary (not the library) to ensure
//!      version alignment with the user's wasm-bindgen dependency.

use std::path::Path;
use std::process::Command;

use tracing::{debug, info};

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Target platform for wasm-bindgen output.
#[derive(Clone, Copy, Debug)]
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
    fn cli_arg(self) -> Option<&'static str> {
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
) -> Result<()> {
    run_wasm_bindgen_with_name(wasm_path, output_dir, target, None)
}

/// Run wasm-bindgen CLI with an optional output name override.
///
/// # Errors
/// Returns an error when the wasm-bindgen CLI is missing or its run fails.
pub fn run_wasm_bindgen_with_name(
    wasm_path: &Path,
    output_dir: &Path,
    target: BindgenTarget,
    out_name: Option<&str>,
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?;

    for entry in std::fs::read_dir(output_dir)
        .map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?
    {
        let entry = entry.map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?;
        if entry.file_type()
            .map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?
            .is_file()
        {
            std::fs::remove_file(entry.path())
                .map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?;
        }
    }

    which::which("wasm-bindgen").map_err(|_| WasmTestbedError::WasmBindgenNotFound.trace())?;

    info!("Running wasm-bindgen...");

    let mut cmd = Command::new("wasm-bindgen");
    cmd.arg(wasm_path)
        .arg("--out-dir")
        .arg(output_dir);

    if let Some(name) = out_name {
        cmd.arg("--out-name").arg(name);
    }

    if let Some(arg) = target.cli_arg() {
        cmd.arg("--target").arg(arg);
    }

    debug!("Executing: {:?}", cmd);

    let status = cmd.status().map_err(|e| {
        WasmTestbedError::WasmBindgenExecFailed(e).trace()
    })?;

    if !status.success() {
        return Err(WasmTestbedError::WasmBindgenFailed(status.code()).trace());
    }

    info!("wasm-bindgen output in {}", output_dir.display());
    Ok(())
}
