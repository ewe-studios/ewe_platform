//! The two cases of the wasm-bindgen regression fixture (spec 88 T5).
//!
//! `a_passes` is green and `b_fails` is red. Every wasm-bindgen runner must
//! exit non-zero when it runs both, and exit 0 when `--filter a_passes` runs
//! only the first. Keep this file the only test target (rule R10).

#![cfg(target_arch = "wasm32")]

use foundation_testbed_bindgen_failing_case::ANSWER;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn a_passes() {
    assert_eq!(ANSWER, 42, "the passing case of the fixture");
}

#[wasm_bindgen_test]
fn b_fails() {
    assert_eq!(1, 2, "b_fails is the deliberate failure of this fixture");
}
