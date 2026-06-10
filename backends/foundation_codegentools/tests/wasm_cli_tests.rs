//! WHY: Feature 15 criterion #4 — the CLI must inspect / validate / convert / edit
//! real `.wasm` and `.wat` files through the typed model, end to end.
//!
//! WHAT: Drives `cli::wasm::{command, run}` exactly as the binary would (clap arg
//! parsing included) against the workspace's real e2e fixture.
//!
//! HOW: Each test parses an argv with `command().get_matches_from`, runs it, and
//! asserts on the produced files (conversion outputs, edited modules).
#![cfg(feature = "cli")]

use std::path::PathBuf;

use foundation_codegen::wasm::sections::payload;
use foundation_codegen::wasm::Module;
use foundation_codegentools::cli::wasm;

fn fixture() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("backends/foundation_wasm/integration/fixtures/foundation_wasm_e2e.wasm");
    path.is_file().then_some(path)
}

fn run(argv: &[&str]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let matches = wasm::command().get_matches_from(argv);
    wasm::run(&matches)
}

#[test]
fn validate_and_inspect_accept_a_real_module() {
    let Some(fixture) = fixture() else {
        eprintln!("fixture not built — skipping");
        return;
    };
    run(&["wasm", "validate", fixture.to_str().unwrap()]).expect("validate succeeds");
    run(&["wasm", "inspect", fixture.to_str().unwrap()]).expect("inspect succeeds");
}

#[test]
fn convert_round_trips_wasm_to_wat_and_back() {
    let Some(fixture) = fixture() else {
        eprintln!("fixture not built — skipping");
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let wat_path = dir.path().join("module.wat");
    let wasm_path = dir.path().join("back.wasm");

    run(&["wasm", "convert", fixture.to_str().unwrap(), wat_path.to_str().unwrap()])
        .expect("wasm -> wat");
    assert!(std::fs::read_to_string(&wat_path)
        .expect("wat written")
        .starts_with("(module"));

    run(&["wasm", "convert", wat_path.to_str().unwrap(), wasm_path.to_str().unwrap()])
        .expect("wat -> wasm");
    run(&["wasm", "validate", wasm_path.to_str().unwrap()]).expect("converted module validates");
}

#[test]
fn edit_add_custom_section_and_rename_export() {
    let Some(fixture) = fixture() else {
        eprintln!("fixture not built — skipping");
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let tagged = dir.path().join("tagged.wasm");
    let renamed = dir.path().join("renamed.wasm");

    run(&[
        "wasm", "edit", "add-custom-section", fixture.to_str().unwrap(),
        "--name", "ewe.cli-test", "--data", "hello",
        "-o", tagged.to_str().unwrap(),
    ])
    .expect("add-custom-section succeeds");
    let original = std::fs::read(&fixture).expect("read original");
    let edited = std::fs::read(&tagged).expect("read edited");
    assert!(edited.starts_with(&original), "edit is append-only (minimal diff)");

    run(&[
        "wasm", "edit", "rename-export", tagged.to_str().unwrap(),
        "--from", "roundtrip_i32", "--to", "roundtrip_i32_renamed",
        "-o", renamed.to_str().unwrap(),
    ])
    .expect("rename-export succeeds");

    let module = Module::decode_from(std::fs::read(&renamed).expect("read").as_slice())
        .expect("renamed module decodes");
    let exports = module
        .find_std_section::<payload::Export>()
        .expect("exports")
        .try_contents()
        .expect("decode exports");
    assert!(exports.iter().any(|e| e.name == "roundtrip_i32_renamed"));
    assert!(!exports.iter().any(|e| e.name == "roundtrip_i32"));
}
