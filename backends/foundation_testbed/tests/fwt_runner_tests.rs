#![cfg(feature = "wasm")]
//! WHY: Feature 12 — the owned runner is the contract contributors rely on
//! (`wasm-testbed node <crate>`): build → discover → stage → run → exit code.
//! These tests pin that loop programmatically against the real sample crate.
//!
//! WHAT: Full red run (the sample contains a deliberate failure → exit 1 with the
//! failure named), filtered green run (exit 0, summary line), and staging contents
//! (self-contained harness: runtime + runner + cases.json + module.wasm).
//!
//! HOW: Uses the library API the CLI itself dispatches to. Requires `node` and a
//! Rust wasm32 target — skips (with a message) when node is unavailable.

use std::path::PathBuf;

use foundation_testbed::wasm::cli::{Browser, OwnedRunArgs};
use foundation_testbed::wasm::fwt_runner::{run_node, stage};

fn sample_crate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("integration/module")
}

fn args(filter: Option<&str>) -> OwnedRunArgs {
    OwnedRunArgs {
        crate_path: sample_crate(),
        release: false,
        features: None,
        filter: filter.map(String::from),
        browser: Browser::Chrome,
        headless: true,
    }
}

fn node_available() -> bool {
    which::which("node").is_ok()
}

#[test]
fn full_run_is_red_with_the_deliberate_failure_named() {
    if !node_available() {
        eprintln!("node not on PATH — skipping");
        return;
    }
    let outcome = run_node(&args(None)).expect("runner executes");
    assert_eq!(outcome.exit_code, 1, "the sample contains a failing case");
    assert_eq!(outcome.cases.len(), 5);
    assert!(outcome.output.contains("FAILED  fails_with_assertion"));
    assert!(outcome.output.contains("ok      passes_simple"));
    assert!(outcome.output.contains("ok      panics_as_expected"), "should_panic inverted");
    assert!(outcome.output.contains("ok      async_completes_after_yield"));
    assert!(outcome.output.contains("ignored ignored_case"));
    assert!(outcome
        .output
        .contains("test result: FAILED. 3 passed; 1 failed; 1 ignored"));
}

#[test]
fn filtered_run_is_green_with_exit_zero() {
    if !node_available() {
        eprintln!("node not on PATH — skipping");
        return;
    }
    let outcome = run_node(&args(Some("passes"))).expect("runner executes");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.cases.len(), 1);
    assert!(outcome
        .output
        .contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
}

#[test]
fn staging_produces_a_self_contained_harness() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cases = stage(&sample_crate(), &args(None), dir.path()).expect("stage succeeds");
    assert_eq!(cases.len(), 5);
    for file in [
        "foundation-wasm.js",
        "foundation-wasm-ui.js",
        "runner.mjs",
        "index.html",
        "cases.json",
        "module.wasm",
    ] {
        assert!(dir.path().join(file).is_file(), "{file} staged");
    }
    let manifest = std::fs::read_to_string(dir.path().join("cases.json")).expect("read");
    assert!(manifest.contains("\"name\":\"passes_simple\""));
    assert!(manifest.contains("\"should_panic\":true"));
}
