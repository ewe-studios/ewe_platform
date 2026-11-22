//! Tests for `include_json!`.
//!
//! The bad-input cases are compile-time errors, so they are not tests here.
//! The build proof is recorded in `specifications/89-mime-types/LEARNINGS.md`.
//! The fixture files in `tests/fixtures/` are its inputs.

use foundation_macros::include_json;

static TYPES: ::foundation_nostd::phf::Map<&'static str, &'static str> =
    include_json!("tests/fixtures/valid.json");

#[test]
fn map_holds_every_pair() {
    assert_eq!(TYPES.len(), 4);
    assert_eq!(TYPES.get("html"), Some(&"text/html"));
    assert_eq!(TYPES.get("js"), Some(&"text/javascript"));
    assert_eq!(TYPES.get("mjs"), Some(&"text/javascript"));
    assert_eq!(TYPES.get("wasm"), Some(&"application/wasm"));
}

#[test]
fn missing_key_returns_none() {
    assert_eq!(TYPES.get("nope"), None);
}
