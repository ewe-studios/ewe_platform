//! wasm binary test discovery via walrus.
//!
//! WHY: wasm-bindgen exports test functions with names starting with `__wbgt_`.
//! WHAT: Parses the wasm binary to find these exports.
//! HOW: Uses walrus (same library as wasm-bindgen-test-runner) to parse
//!      the wasm module and iterate the export section.

use std::path::Path;

use tracing::debug;

use crate::error::{Result, ToTrace, WasmTestbedError};

/// Discover `#[wasm_bindgen_test]` test functions in a wasm binary.
///
/// The wasm binary should be the wasm-bindgen output (e.g. `{name}_bg.wasm`
/// from the bindgen output directory), not the raw cargo build output.
///
/// Returns a sorted list of test export names (e.g. `["__wbgt_test_add", "__wbgt_test_sub"]`).
///
/// # Errors
///
/// Returns an error if:
/// - the wasm file doesn't exist
/// - the wasm binary cannot be parsed
pub fn discover_tests(wasm_path: &Path) -> Result<Vec<String>> {
    if !wasm_path.exists() {
        return Err(WasmTestbedError::TestWasmNotFound(wasm_path.display().to_string()).trace());
    }

    let bytes = std::fs::read(wasm_path)
        .map_err(|e| WasmTestbedError::WasmReadFailed(e).trace())?;

    let module = walrus::ModuleConfig::new()
        .parse(&bytes)
        .map_err(|e| WasmTestbedError::WasmParseFailed(e.to_string()).trace())?;

    let mut tests: Vec<String> = module
        .exports
        .iter()
        .filter(|export| export.name.starts_with("__wbgt_"))
        .map(|export| export.name.clone())
        .collect();

    tests.sort();
    debug!("Discovered {} tests: {:?}", tests.len(), tests);

    Ok(tests)
}
