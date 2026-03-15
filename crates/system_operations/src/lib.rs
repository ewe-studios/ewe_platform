/// WHY: WASM binary generation needs a centralized module so the CLI,
/// tests, and build scripts can all use the same logic.
///
/// WHAT: Re-exports the `wasm_bins` module for WASM binary entrypoint
/// scanning, planning, and generation.
///
/// HOW: Delegates to the `wasm_bins` submodule which contains all types
/// and the `WasmBinGenerator` orchestrator.
pub mod wasm_bins;
