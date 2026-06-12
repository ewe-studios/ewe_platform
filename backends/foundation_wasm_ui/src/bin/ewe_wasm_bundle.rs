//! WHY: The crate that defines the UI runtime owns how it ships (feature 20)
//! — including a standalone CLI, so `cargo install foundation_wasm_ui` is all
//! a consumer needs.
//!
//! WHAT: `ewe-wasm-bundle` — the `foundation_wasm_ui::cli` surface
//! (`build`, `plan`) as its own binary.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    let matches = foundation_wasm_ui::cli::command().get_matches();
    if let Err(error) = foundation_wasm_ui::cli::run(&matches) {
        tracing::error!("ewe-wasm-bundle: {error}");
        std::process::exit(1);
    }
}

// Cargo builds every target on `--target wasm32-*` too; the CLI is native
// tooling, so the wasm build degrades to a stub.
#[cfg(target_arch = "wasm32")]
fn main() {}
