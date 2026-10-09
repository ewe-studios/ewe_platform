//! WASI runner — build for `wasm32-wasip1`/`wasm32-wasip2`, run via `wasmtime`.
//!
//! Subprocess runner: compiles the crate with the WASI target, then executes
//! the resulting `.wasm` binary under the `wasmtime` CLI. This is a
//! build-and-verify runner — it confirms that the crate compiles and runs
//! under a WASI host. The full in-process WASI runner (using the wasmtime
//! Rust API) is owned by `foundation_wasmtime` (Phase 4).

use std::path::PathBuf;
use std::process::Command;

use tracing::{debug, info};

use crate::wasm::build;
use crate::wasm::cli::{WasiArgs, WasmTarget};
use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Build a crate for the selected WASI target and run via `wasmtime`.
///
/// # Errors
/// Returns an error if cargo/wasmtime is missing, the build fails, or wasmtime
/// exits non-zero.
pub fn run(args: &WasiArgs) -> Result<RunOutput> {
    let target_triple = match args.target {
        WasmTarget::Wasip1 => "wasm32-wasip1",
        WasmTarget::Wasip2 => "wasm32-wasip2",
        other => {
            return Err(WasmTestbedError::WasiTargetRequired(other.triple().to_string()).trace());
        }
    };

    preflight()?;
    let crate_path = canonical(&args.crate_path)?;

    let built = build::run_for_target(
        &crate_path,
        args.release,
        args.features.as_deref(),
        target_triple,
    )?;

    info!(
        "running WASI module via wasmtime: {}",
        built.wasm_path.display()
    );

    let mut cmd = Command::new("wasmtime");
    cmd.arg("run");

    if matches!(args.target, WasmTarget::Wasip2) {
        cmd.arg("--wasm").arg("component-model=y");
    }

    cmd.arg(&built.wasm_path);

    debug!("Executing: {:?}", cmd);

    let output = cmd
        .output()
        .map_err(|e| WasmTestbedError::WasiExecFailed(e).trace())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        debug!("wasmtime stderr:\n{stderr}");
    }

    if exit_code != 0 {
        return Err(WasmTestbedError::WasiRunFailed(exit_code, stdout, stderr).trace());
    }

    info!(
        "WASI run succeeded ({}, target {})",
        built.package_name, target_triple
    );

    Ok(RunOutput {
        stdout,
        stderr,
        exit_code,
        package_name: built.package_name,
    })
}

pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub package_name: String,
}

fn preflight() -> Result<()> {
    which::which("cargo").map_err(|_| WasmTestbedError::CargoNotFound.trace())?;
    which::which("wasmtime").map_err(|_| {
        WasmTestbedError::MissingTool {
            tool: "wasmtime".to_string(),
            why: "runs WASI modules (install: curl https://wasmtime.dev/install.sh -sSf | bash)"
                .to_string(),
        }
        .trace()
    })?;
    Ok(())
}

fn canonical(path: &PathBuf) -> Result<PathBuf> {
    std::fs::canonicalize(path)
        .map_err(|_| WasmTestbedError::CratePathNotFound(path.display().to_string()).trace())
}
