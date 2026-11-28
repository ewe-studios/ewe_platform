//! The browser tests of `foundation_auth` (spec 92 feature 00, task T6).
//!
//! This target builds only for wasm32 and runs in a browser page. The
//! `wasm-testbed` tool discovers the `#[wasm_bindgen_test]` cases.

#![cfg(target_arch = "wasm32")]

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

mod webcrypto_sealer_tests;
