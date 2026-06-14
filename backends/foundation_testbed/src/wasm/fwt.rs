//! WHY: Feature 13 — the testbed must enumerate `#[wasm_test]` cases (and their
//! flags) from a compiled module WITHOUT running it, on the OWNED wasm model.
//!
//! WHAT: [`discover_cases`] — reads `__fwt_*` function exports plus the
//! `__fwt_manifest` custom section (`name|flags\n` lines; flags: `a`sync,
//! `p`anic-expected, `i`gnored) from the RAW cargo wasm output.
//!
//! HOW: Uses our `foundation_codegen::wasm` model (the F15 wasmbin port) instead of
//! walrus — the spec predates F15; decision 031 prefers the owned reader, and F15
//! was built with this exact use case as a success criterion. The export scan is
//! authoritative for WHICH cases exist; the manifest enriches them with flags.

use std::collections::BTreeMap;
use std::path::Path;

use foundation_codegen::wasm::sections::{ExportDesc, Section};
use foundation_codegen::wasm::Module;
use tracing::debug;

use crate::wasm::error::{Result, ToTrace, WasmTestbedError};

/// Prefix every `#[wasm_test]` export carries (ours — the owned counterpart of the
/// wasm-bindgen `__wbgt_` convention).
pub const FWT_PREFIX: &str = "__fwt_";

/// Name of the custom section holding `name|flags` manifest lines.
pub const FWT_MANIFEST_SECTION: &str = "__fwt_manifest";

/// One discovered `#[wasm_test]` case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FwtCase {
    /// The case (function) name.
    pub name: String,
    /// The export to call (`__fwt_<name>`).
    pub export: String,
    /// `async fn` case — the report arrives via `host_report` after the export returns 1.
    pub is_async: bool,
    /// `#[wasm_test(should_panic)]` — the runner inverts the verdict.
    pub should_panic: bool,
    /// `#[wasm_test(ignore)]` — reported as skipped without running the body.
    pub ignore: bool,
}

/// Discover `#[wasm_test]` cases in the RAW cargo wasm output (no bindgen step).
///
/// # Errors
/// Fails when the file is missing/unreadable or is not a valid wasm module.
pub fn discover_cases(wasm_path: &Path) -> Result<Vec<FwtCase>> {
    if !wasm_path.exists() {
        return Err(WasmTestbedError::TestWasmNotFound(wasm_path.display().to_string()).trace());
    }
    let bytes =
        std::fs::read(wasm_path).map_err(|e| WasmTestbedError::WasmReadFailed(e).trace())?;
    let module = Module::decode_from(bytes.as_slice())
        .map_err(|e| WasmTestbedError::WasmParseFailed(e.to_string()).trace())?;

    let flags = manifest_flags(&module);

    let mut cases: Vec<FwtCase> = Vec::new();
    for section in &module.sections {
        let Section::Export(blob) = section else {
            continue;
        };
        let exports = blob
            .try_contents()
            .map_err(|e| WasmTestbedError::WasmParseFailed(e.to_string()).trace())?;
        for export in exports {
            if !matches!(export.desc, ExportDesc::Func(_)) {
                continue;
            }
            let Some(name) = export.name.strip_prefix(FWT_PREFIX) else {
                continue;
            };
            let case_flags = flags.get(name).map_or("", String::as_str);
            cases.push(FwtCase {
                name: name.to_string(),
                export: export.name.clone(),
                is_async: case_flags.contains('a'),
                should_panic: case_flags.contains('p'),
                ignore: case_flags.contains('i'),
            });
        }
    }

    cases.sort_by(|a, b| a.name.cmp(&b.name));
    debug!("Discovered {} #[wasm_test] cases: {:?}", cases.len(), cases);
    Ok(cases)
}

/// Read `name -> flags` out of the `__fwt_manifest` custom section, if present.
fn manifest_flags(module: &Module) -> BTreeMap<String, String> {
    let mut flags = BTreeMap::new();
    for section in &module.sections {
        let Section::Custom(blob) = section else {
            continue;
        };
        let Ok(custom) = blob.try_contents() else {
            continue;
        };
        if custom.name() != FWT_MANIFEST_SECTION {
            continue;
        }
        // The section payload is the concatenated `name|flags\n` statics. The
        // CustomSection::Other raw form keeps the bytes verbatim.
        let foundation_codegen::wasm::sections::CustomSection::Other(raw) = custom else {
            continue;
        };
        for line in raw.data.bytes.split(|byte| *byte == b'\n') {
            let Ok(line) = std::str::from_utf8(line) else {
                continue;
            };
            if let Some((name, case_flags)) = line.split_once('|') {
                flags.insert(name.to_string(), case_flags.to_string());
            }
        }
    }
    flags
}
