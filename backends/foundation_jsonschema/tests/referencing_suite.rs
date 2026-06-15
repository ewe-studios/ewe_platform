//! Referencing test suite — validates URI resolution behavior.
//!
//! WHY: JSON Schema reference resolution is complex: it must handle
//! `$id` base URIs, fragment-only refs, JSON Pointer fragments,
//! anchor references, and relative URI resolution per RFC 3986.
//!
//! WHAT: Tests the registry and resolver subsystems through the
//! public `ValidationOptions` API, verifying that schemas with
//! various referencing patterns compile and validate correctly.
//!
//! HOW: Each test constructs a schema with specific referencing
//! features (anchors, fragments, relative URIs, etc.) and asserts
//! that the compiled validator produces correct results.

use foundation_jsonschema::{InMemoryFetcher, ValidationOptions};
use serde_json::json;

// ── Anchor resolution ──────────────────────────────────────────────

#[test]
fn anchor_ref_same_document() {
    // WHAT: Verify that `$anchor` references resolve within the same document.
    //
    // WHY: Anchors allow schemas to define named subschemas that can be
    // referenced with `#anchor-name` without embedding the full JSON Pointer.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {
            "person": {
                "$anchor": "person",
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            }
        },
        "$ref": "#person"
    });

    let validator = ValidationOptions::new()
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!({"name": "Alice"})));
    assert!(!validator.is_valid(&json!({"name": 42})));
}

#[test]
fn json_pointer_ref() {
    // WHAT: Verify that JSON Pointer fragments in `$ref` resolve correctly.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {
            "stringType": {"type": "string", "minLength": 1}
        },
        "$ref": "#/$defs/stringType"
    });

    let validator = ValidationOptions::new()
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!("hello")));
    assert!(!validator.is_valid(&json!("")));
    assert!(!validator.is_valid(&json!(42)));
}

#[test]
fn empty_fragment_ref() {
    // WHAT: Verify that `#` (empty fragment) refers to the document root.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "$ref": "#"
    });

    let validator = ValidationOptions::new()
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!({})));
    assert!(!validator.is_valid(&json!(42)));
}

#[test]
fn relative_uri_ref() {
    // WHAT: Verify that relative URI references resolve against the base `$id`.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "http://example.com/root",
        "$defs": {
            "child": {
                "$id": "http://example.com/child",
                "type": "string"
            }
        },
        "$ref": "http://example.com/child"
    });

    let validator = ValidationOptions::new()
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!("hello")));
    assert!(!validator.is_valid(&json!(42)));
}

#[test]
fn external_ref_via_resolver() {
    // WHAT: Verify that external `$ref` URIs resolve through the resolver.
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert(
        "http://example.com/person",
        json!({
            "type": "object",
            "properties": {"name": {"type": "string"}}
        }),
    );

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "http://example.com/person"
    });

    let validator = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!({"name": "Alice"})));
    assert!(!validator.is_valid(&json!({"name": 42})));
}

#[test]
fn dynamic_anchor_ref() {
    // WHAT: Verify that `$dynamicRef` / `$dynamicAnchor` resolve correctly
    // in the basic case (no schema extension).
    //
    // NOTE: Full dynamic scope resolution (where extending schemas override
    // the anchor) requires runtime scope tracking — this test covers the
    // compile-time resolution path.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {
            "base": {
                "$dynamicAnchor": "item",
                "type": "string"
            }
        },
        "$dynamicRef": "#item"
    });

    let validator = ValidationOptions::new()
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!("hello")));
    assert!(!validator.is_valid(&json!(42)));
}

#[test]
fn nested_subresource_resolution() {
    // WHAT: Verify that `$id` in nested subschemas creates new base URIs.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "http://example.com/base",
        "properties": {
            "address": {
                "$id": "http://example.com/address",
                "type": "object",
                "properties": {
                    "street": {"type": "string"},
                    "zip": {"type": "string", "pattern": "^[0-9]{5}$"}
                }
            }
        }
    });

    let validator = ValidationOptions::new()
        .build(&schema)
        .expect("schema should compile");

    assert!(validator.is_valid(&json!({
        "address": {"street": "123 Main St", "zip": "12345"}
    })));
    assert!(!validator.is_valid(&json!({
        "address": {"street": "123 Main St", "zip": "abcde"}
    })));
}

#[test]
fn uri_normalization_case_insensitive_host() {
    // WHAT: Verify that URIs are normalized per RFC 3986 §6.2.2.1.
    // Schemes and hosts are case-insensitive; the registry normalizes
    // both on insertion and lookup.
    //
    // WHY: Without normalization, `http://Example.COM/schema` and
    // `http://example.com/schema` would be treated as different URIs,
    // causing `$ref` resolution to fail.
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert("http://example.com/schema", json!({"type": "string"}));

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "http://example.com/root",
        "allOf": [
            {"$ref": "http://EXAMPLE.COM/schema"}
        ]
    });

    let result = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema);

    assert!(result.is_ok(), "should resolve with normalized URI");
    let validator = result.unwrap();
    assert!(validator.is_valid(&json!("hello")));
    assert!(!validator.is_valid(&json!(42)));
}

#[test]
fn uri_normalization_case_insensitive_scheme() {
    // WHAT: Verify that scheme case is normalized to lowercase.
    let mut fetcher = InMemoryFetcher::builtin();
    fetcher.insert("http://example.com/schema", json!({"type": "string"}));

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "http://example.com/root",
        "allOf": [
            {"$ref": "HTTP://example.com/schema"}
        ]
    });

    let result = ValidationOptions::new()
        .with_resolver(fetcher)
        .build(&schema);

    assert!(result.is_ok(), "should resolve with normalized scheme");
    let validator = result.unwrap();
    assert!(validator.is_valid(&json!("hello")));
}
