//! Builder fuzz target — schema compilation.
//!
//! WHY: Schema compilation processes arbitrary user-provided JSON. It must
//! never panic, regardless of input — malformed JSON, unexpected types,
//! circular references, or absurdly deep nesting must all be handled
//! gracefully by returning an error.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(schema) = serde_json::from_slice::<serde_json::Value>(data) {
        // We don't care if compilation succeeds or fails — we only care
        // that it doesn't panic.
        let _ = foundation_jsonschema::validator_for(&schema);
    }
});
