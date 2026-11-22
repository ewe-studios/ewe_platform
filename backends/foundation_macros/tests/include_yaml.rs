//! Tests for `include_yaml!`.
//!
//! The bad-input cases are compile-time errors, so they are not tests here.
//! The build proof is recorded in `specifications/89-mime-types/LEARNINGS.md`.
//! The fixture files in `tests/fixtures/` are its inputs.

use foundation_macros::include_yaml;

static TYPES: ::foundation_nostd::phf::Map<&'static str, &'static str> =
    include_yaml!("tests/fixtures/valid.yaml");

static SCALAR_KEYS: ::foundation_nostd::phf::Map<&'static str, &'static str> =
    include_yaml!("tests/fixtures/scalar_key.yaml");

#[test]
fn mapping_holds_every_pair() {
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

#[test]
fn scalar_key_becomes_its_text() {
    assert_eq!(SCALAR_KEYS.get("3"), Some(&"three"));
    assert_eq!(SCALAR_KEYS.get("true"), Some(&"yes"));
    assert_eq!(SCALAR_KEYS.get("answer"), Some(&"forty-two"));
}
