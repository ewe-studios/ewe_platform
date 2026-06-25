//! Emscripten runner — build for `wasm32-unknown-emscripten`, run via the
//! embedded Deno runtime (same FWT protocol as the `deno` subcommand).
//!
//! This is a convenience entry point: it constructs an `OwnedRunArgs` with
//! `target = Emscripten` and delegates to `fwt_runner::run_deno`. The embedded
//! Deno runtime (spec-44) loads and executes the wasm module — no external
//! node/deno binary required.

use crate::wasm::cli::{Browser, EmscriptenArgs, OwnedRunArgs, WasmTarget};
use crate::wasm::fwt_runner::{self, RunOutcome};
use crate::wasm::error::Result;

/// Build a crate for `wasm32-unknown-emscripten` and run via the embedded Deno
/// runtime using the FWT test protocol.
///
/// # Errors
/// Returns an error if the build fails or the embedded runtime run errors.
pub fn run(args: &EmscriptenArgs) -> Result<RunOutcome> {
    let owned = OwnedRunArgs {
        crate_path: args.crate_path.clone(),
        target: WasmTarget::Emscripten,
        release: args.release,
        features: args.features.clone(),
        filter: None,
        browser: Browser::Chrome,
        headless: true,
    };
    fwt_runner::run_deno(&owned)
}
