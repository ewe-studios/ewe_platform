//! Emscripten runner — build for `wasm32-unknown-emscripten`, run via `node`.
//!
//! Subprocess runner: compiles the crate with the emscripten target, then
//! executes the resulting JS glue file under Node.js. The emscripten linker
//! produces both a `.wasm` and a `.js` loader; node runs the `.js` file which
//! bootstraps the wasm module with emscripten's libc/syscall layer.

use std::path::PathBuf;
use std::process::Command;

use tracing::{debug, info};

use crate::wasm::build;
use crate::wasm::cli::EmscriptenArgs;
use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Build a crate for `wasm32-unknown-emscripten` and run via node.
///
/// # Errors
/// Returns an error if cargo/node is missing, the build fails, or node exits
/// non-zero.
pub fn run(args: &EmscriptenArgs) -> Result<RunOutput> {
    preflight()?;
    let crate_path = canonical(&args.crate_path)?;
    let target_triple = "wasm32-unknown-emscripten";

    let built = build::run_for_target(
        &crate_path,
        args.release,
        args.features.as_deref(),
        target_triple,
    )?;

    // Emscripten cdylib produces <name>.js alongside <name>.wasm.
    // The .js file is the entry point that bootstraps the wasm module.
    let js_path = built.wasm_path.with_extension("js");
    if !js_path.exists() {
        info!(
            "no emscripten JS glue at {} — running wasm directly is not supported for emscripten cdylibs",
            js_path.display()
        );
        return Err(WasmTestbedError::EmscriptenJsNotFound(
            js_path.display().to_string(),
        )
        .trace());
    }

    info!(
        "running emscripten module via node: {}",
        js_path.display()
    );

    let mut cmd = Command::new("node");
    cmd.arg(&js_path);

    if let Some(parent) = js_path.parent() {
        cmd.current_dir(parent);
    }

    debug!("Executing: {:?}", cmd);

    let output = cmd
        .output()
        .map_err(|e| WasmTestbedError::EmscriptenExecFailed(e).trace())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        debug!("node stderr:\n{stderr}");
    }

    if exit_code != 0 {
        return Err(WasmTestbedError::EmscriptenRunFailed(exit_code, stdout, stderr).trace());
    }

    info!("emscripten run succeeded ({})", built.package_name);

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
    which::which("node").map_err(|_| {
        WasmTestbedError::MissingTool {
            tool: "node".to_string(),
            why: "runs the emscripten JS glue that bootstraps the wasm module".to_string(),
        }
        .trace()
    })?;
    Ok(())
}

fn canonical(path: &PathBuf) -> Result<PathBuf> {
    std::fs::canonicalize(path)
        .map_err(|_| WasmTestbedError::CratePathNotFound(path.display().to_string()).trace())
}
