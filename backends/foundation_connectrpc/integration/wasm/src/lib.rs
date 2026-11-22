//! Test support for the wasm streaming path of foundation_connectrpc and
//! foundation_http.
//!
//! WHY: foundation_connectrpc's test targets cannot build for wasm32 (see the
//! package comment in `Cargo.toml`), and the streaming request bridge of
//! foundation_http has no wasm test runner of its own.
//!
//! WHAT: this crate exists so the wasm testbed can build and run the tests in
//! `tests/`. The tests live there; the library is empty.

/// Nothing to export: the tests are the payload of this crate.
pub fn crate_marker() {}
