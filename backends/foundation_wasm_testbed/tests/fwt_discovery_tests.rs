//! WHY: Feature 13 — the testbed must enumerate `#[wasm_test]` cases (names +
//! async/should_panic/ignore flags) from a compiled module WITHOUT running it,
//! using the owned wasm reader (the F15 wasmbin port), not walrus and not
//! wasm-bindgen.
//!
//! WHAT: Drives `fwt::discover_cases` against the real `fwt_sample.wasm` fixture
//! built from `integration/module` (skips when the fixture isn't built).
//!
//! HOW: The fixture declares five cases covering every flag combination the macro
//! emits; discovery must surface exactly those, sorted, with the right flags.

use std::path::PathBuf;

use foundation_wasm_testbed::fwt::{discover_cases, FwtCase};

fn fixture() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("integration/fixtures/fwt_sample.wasm");
    path.is_file().then_some(path)
}

#[test]
fn discovers_all_cases_with_flags() {
    let Some(path) = fixture() else {
        eprintln!("fixture not built (run integration/build-module.sh) — skipping");
        return;
    };
    let cases = discover_cases(&path).expect("discovery succeeds");

    let expected = vec![
        FwtCase {
            name: "async_completes_after_yield".into(),
            export: "__fwt_async_completes_after_yield".into(),
            is_async: true,
            should_panic: false,
            ignore: false,
        },
        FwtCase {
            name: "fails_with_assertion".into(),
            export: "__fwt_fails_with_assertion".into(),
            is_async: false,
            should_panic: false,
            ignore: false,
        },
        FwtCase {
            name: "ignored_case".into(),
            export: "__fwt_ignored_case".into(),
            is_async: false,
            should_panic: false,
            ignore: true,
        },
        FwtCase {
            name: "panics_as_expected".into(),
            export: "__fwt_panics_as_expected".into(),
            is_async: false,
            should_panic: true,
            ignore: false,
        },
        FwtCase {
            name: "passes_simple".into(),
            export: "__fwt_passes_simple".into(),
            is_async: false,
            should_panic: false,
            ignore: false,
        },
    ];
    assert_eq!(cases, expected);
}

#[test]
fn missing_file_is_a_clear_error() {
    let err = discover_cases(std::path::Path::new("/nonexistent/never.wasm"))
        .expect_err("missing file must error");
    let text = format!("{err:?}");
    assert!(text.contains("never.wasm"), "error names the file: {text}");
}
