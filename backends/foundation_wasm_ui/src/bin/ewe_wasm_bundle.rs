//! WHY: The crate that defines the UI runtime owns how it ships (feature 20)
//! — including a standalone CLI, so `cargo install foundation_wasm_ui
//! --features cli` is all a consumer needs.
//!
//! WHAT: `ewe-wasm-bundle` — the `foundation_wasm_ui::cli` surface
//! (`build`, `plan`) as its own binary.

fn main() {
    let matches = foundation_wasm_ui::cli::command().get_matches();
    if let Err(error) = foundation_wasm_ui::cli::run(&matches) {
        eprintln!("ewe-wasm-bundle: {error}");
        std::process::exit(1);
    }
}
