//! Referencing fuzz target — URI/pointer/registry resolution.
//!
//! WHY: URI parsing and JSON Pointer traversal process arbitrary strings.
//! Edge cases in percent-encoding, fragment resolution, and anchor lookup
//! could cause panics. The registry builder also crawls arbitrary JSON
//! looking for $id, $ref, and anchors — it must handle malformed input.

#![no_main]

use foundation_jsonschema::referencing::Registry;
use foundation_jsonschema::Draft;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }
    let schema_end = data.len() / 3;
    let uri_end = 2 * data.len() / 3;

    let Ok(schema) = serde_json::from_slice::<serde_json::Value>(&data[..schema_end]) else {
        return;
    };
    let Ok(base_uri) = core::str::from_utf8(&data[schema_end..uri_end]) else {
        return;
    };
    let Ok(reference) = core::str::from_utf8(&data[uri_end..]) else {
        return;
    };

    for draft in [
        Draft::Draft4,
        Draft::Draft6,
        Draft::Draft7,
        Draft::Draft201909,
        Draft::Draft202012,
    ] {
        let registry = Registry::builder()
            .with_draft(draft)
            .add_resource(base_uri, schema.clone())
            .build();

        if let Ok(registry) = registry {
            let resolver = registry.resolver(base_uri);
            let _ = resolver.lookup(reference);
        }
    }
});
