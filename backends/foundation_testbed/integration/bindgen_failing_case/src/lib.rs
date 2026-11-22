//! The lib of the wasm-bindgen regression fixture (spec 88 feature 00 T5).
//!
//! WHY: the testbed's wasm-bindgen runners must report a failing case. This
//! crate holds one passing case and one failing case, so a runner that reports
//! the truth turns red (rule R14).
//!
//! WHAT: the crate is a `cdylib` and an `rlib`, with the cases in `tests/`.
//! HOW: `cargo build --tests --target wasm32-unknown-unknown` compiles the
//! `#[wasm_bindgen_test]` cases into the test module the runners execute.

/// A value the fixture's passing case checks.
pub const ANSWER: i32 = 42;
