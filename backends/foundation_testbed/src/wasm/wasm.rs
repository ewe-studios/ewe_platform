//! Wasm binary location and wasm-bindgen CLI invocation.
//!
//! WHY: wasm-bindgen must be run to generate JS glue code from wasm binaries.
//! WHAT: Locates wasm-bindgen on PATH, invokes it as a subprocess, and patches
//!      the start call in the glue for a wasm test binary.
//! HOW: Uses the wasm-bindgen CLI binary (not the library) to ensure
//!      version alignment with the user's wasm-bindgen dependency.

use std::path::Path;
use std::process::Command;

use tracing::{debug, info};

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// The start-function call that the wasm-bindgen CLI writes into its JS glue.
const START_CALL: &str = "wasm.__wbindgen_start();";

/// What [`START_CALL`] becomes in a test binary's glue.
///
/// The module start function does two jobs. It initialises the externref table,
/// and it calls the binary's `main`. In a test binary, `main` is libtest's test
/// harness. That harness runs every plain `#[test]` case, and the native-only
/// cases spawn threads, which aborts the wasm module before any
/// `#[wasm_bindgen_test]` case runs. wasm-bindgen's own test runner drops the
/// call (`emit_start(false)`); the CLI has no flag for that. Keep the externref
/// table initialisation, because a `JsValue` constant maps to a wrong table
/// slot without it, and drop only the call to `main`.
const START_CALL_REPLACEMENT: &str = "const __initTable = Object.values(__wbg_get_imports())\n  .map((m) => m && m.__wbindgen_init_externref_table)\n  .find((f) => f);\nif (__initTable) __initTable();";

/// Patch the start call in a wasm-bindgen glue file.
///
/// A runner that calls the raw `__wbgt_` exports needs the `wasm` instance
/// exported, because the glue keeps it private. Set `export_wasm` for such a
/// runner.
///
/// # Errors
///
/// Returns an error when the glue cannot be read or written, or when it has no
/// [`START_CALL`]. A changed CLI output must fail here, not leave the libtest
/// `main` in place and abort the module at start.
pub fn patch_bindgen_glue(js_file: &Path, export_wasm: bool) -> Result<()> {
    let content = std::fs::read_to_string(js_file).map_err(|e| WasmTestbedError::Io(e).trace())?;
    if !content.contains(START_CALL) {
        return Err(WasmTestbedError::WasmParseFailed(format!(
            "the wasm-bindgen glue at {} has no `{START_CALL}`; update the testbed's \
             start-call patch for the installed wasm-bindgen version",
            js_file.display()
        ))
        .trace());
    }

    let mut patched = content.replace(START_CALL, START_CALL_REPLACEMENT);
    if export_wasm {
        patched.push_str("\nexport { wasm };\n");
    }
    std::fs::write(js_file, patched).map_err(|e| WasmTestbedError::Io(e).trace())
}

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
    /// The CLI argument for this target.
    fn cli_arg(self) -> &'static str {
        match self {
            BindgenTarget::Web => "web",
            BindgenTarget::Deno => "deno",
            BindgenTarget::EsModules => "module",
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
pub fn run_wasm_bindgen(wasm_path: &Path, output_dir: &Path, target: BindgenTarget) -> Result<()> {
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
        if entry
            .file_type()
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
    cmd.arg(wasm_path).arg("--out-dir").arg(output_dir);

    if let Some(name) = out_name {
        cmd.arg("--out-name").arg(name);
    }

    cmd.arg("--target").arg(target.cli_arg());

    debug!("Executing: {:?}", cmd);

    let status = cmd
        .status()
        .map_err(|e| WasmTestbedError::WasmBindgenExecFailed(e).trace())?;

    if !status.success() {
        return Err(WasmTestbedError::WasmBindgenFailed(status.code()).trace());
    }

    info!("wasm-bindgen output in {}", output_dir.display());
    Ok(())
}
