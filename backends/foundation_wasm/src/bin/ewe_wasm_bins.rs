//! WHY: The crate that defines the WASM ABI owns how its entrypoints become
//! binaries (feature 20) — including a standalone CLI, so
//! `cargo install foundation_wasm --features cli` is all a consumer needs.
//!
//! WHAT: `ewe-wasm-bins` — the `foundation_wasm::cli` surface (`list`,
//! `generate`) as its own binary.

fn main() {
    let matches = foundation_wasm::cli::command().get_matches();
    if let Err(error) = foundation_wasm::cli::run(&matches) {
        eprintln!("ewe-wasm-bins: {error}");
        std::process::exit(1);
    }
}
