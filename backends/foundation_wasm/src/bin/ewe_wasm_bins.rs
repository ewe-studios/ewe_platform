//! WHY: The crate that defines the WASM ABI owns how its entrypoints become
//! binaries (feature 20) — including a standalone CLI, so
//! `cargo install foundation_wasm` is all a consumer needs.
//!
//! WHAT: `ewe-wasm-bins` — the `foundation_wasm::cli` surface (`list`,
//! `generate`) as its own binary.

#[cfg(not(target_family = "wasm"))]
fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    let matches = foundation_wasm::cli::command().get_matches();
    if let Err(error) = foundation_wasm::cli::run(&matches) {
        tracing::error!("ewe-wasm-bins: {error}");
        std::process::exit(1);
    }
}

// Cargo builds every target on `--target wasm32-*` too; the CLI is native
// tooling, so the wasm build degrades to a stub.
#[cfg(target_family = "wasm")]
fn main() {}
