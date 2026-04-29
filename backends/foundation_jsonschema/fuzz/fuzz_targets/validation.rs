//! Validation fuzz target — all validation code paths.
//!
//! WHY: The validation engine traverses both the compiled schema tree and
//! the instance data. Edge cases in recursion, type coercion, and error
//! collection could cause panics. We exercise all available validation
//! methods to catch issues in any code path.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mid = data.len() / 2;
    if mid == 0 || mid == data.len() {
        return;
    }
    let (schema_bytes, instance_bytes) = data.split_at(mid);

    let Ok(schema) = serde_json::from_slice::<serde_json::Value>(schema_bytes) else {
        return;
    };
    let Ok(instance) = serde_json::from_slice::<serde_json::Value>(instance_bytes) else {
        return;
    };

    let Ok(validator) = foundation_jsonschema::validator_for(&schema) else {
        return;
    };

    // Exercise all validation code paths
    let _ = validator.is_valid(&instance);
    let _ = validator.validate(&instance);
    let _ = validator.iter_errors(&instance).count();
});
