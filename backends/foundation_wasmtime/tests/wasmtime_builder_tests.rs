//! `WasmtimeBuilder` tests moved out of `src/lib.rs` (spec 70 feature 59,
//! rule 7).
//!
//! The test read the private `name` field. The builder now has a public
//! `name` getter, and the assertion is the same.

use foundation_context::Ctx;
use foundation_wasmtime::WasmtimeBuilder;

#[test]
fn builder_creates_from_bytes() {
    let builder = WasmtimeBuilder::new(b"fake wasm bytes").with_name("test");
    assert_eq!(builder.name(), "test");
}

#[test]
fn builder_from_file_missing_returns_error() {
    let result = WasmtimeBuilder::from_file("/tmp/definitely_not_a_wasm_file.wasm");
    assert!(result.is_err());
}

#[test]
fn invalid_wasm_returns_error_on_build() {
    let builder = WasmtimeBuilder::new(b"not valid wasm");
    // The clock and randomness do not matter here: the bytes never compile.
    let result = builder.build(&Ctx::host());
    assert!(result.is_err());
}
