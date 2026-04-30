//! Integration tests — tuple validation, prefixItems+items, and contentSchema.
//!
//! Tests for:
//! - `items` as array (tuple validation) in Draft 4/6/7/2019-09
//! - `additionalItems` with true/false/schema forms
//! - Draft 2020-12 `prefixItems` + `items` interaction
//! - `contentSchema` with encoding and media type combinations

use foundation_jsonschema::{Draft, ValidationOptions};
use serde_json::json;

// ── Tuple validation (items as array) ──────────────────────────────

#[test]
fn tuple_draft7_basic() {
    // Person as tuple: [name (string), age (integer), active (boolean)]
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "integer"},
            {"type": "boolean"}
        ]
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["Alice", 30, true])));
    assert!(!validator.is_valid(&json!([30, "Alice", true]))); // wrong types
}

#[test]
fn tuple_draft7_with_additional_items_true() {
    // Tuple of 2, additionalItems: true (default) allows extra
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "string"}
        ],
        "additionalItems": true
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["first", "last", "extra", 42, true])));
}

#[test]
fn tuple_draft7_with_additional_items_false() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "string"}
        ],
        "additionalItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["first", "last"])));
    assert!(!validator.is_valid(&json!(["first", "last", "extra"])));
}

#[test]
fn tuple_draft7_with_additional_items_schema() {
    // Tuple of 2, extras must be strings
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "string"}
        ],
        "additionalItems": {"type": "string"}
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["first", "last", "extra"])));
    assert!(!validator.is_valid(&json!(["first", "last", 42]))); // 42 not a string
}

#[test]
fn tuple_draft7_empty_array_valid() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    // Empty array is valid (no items to check against tuple)
    assert!(validator.is_valid(&json!([])));
}

#[test]
fn tuple_draft7_fewer_items_than_schema() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["Alice"]))); // only first item
}

#[test]
fn tuple_draft6_basic() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-06/schema#",
        "items": [
            {"type": "string"},
            {"const": 42}
        ],
        "additionalItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft6)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["answer", 42])));
    assert!(!validator.is_valid(&json!(["answer", 43])));
}

#[test]
fn tuple_draft4_with_id_keyword() {
    // Draft 4 uses "id" instead of "$id"
    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#",
        "id": "https://example.com/tuple",
        "items": [
            {"type": "number"},
            {"type": "string"}
        ],
        "additionalItems": {"type": "number"}
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft4)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!([1.5, "hello", 3.14])));
    assert!(!validator.is_valid(&json!([1.5, "hello", "extra"]))); // extra not number
}

#[test]
fn tuple_draft2019_09_basic() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "items": [
            {"type": "string"},
            {"type": "number"}
        ],
        "additionalItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft201909)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["name", 3.14])));
    assert!(!validator.is_valid(&json!(["name", 3.14, "extra"])));
}

// ── items: false (empty tuple) ─────────────────────────────────────

#[test]
fn items_false_draft7_rejects_all() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!([])));
    assert!(!validator.is_valid(&json!([1])));
    assert!(!validator.is_valid(&json!(["anything"])));
}

// ── Draft 2020-12 prefixItems + items interaction ──────────────────

#[test]
fn draft2020_12_prefix_items_with_items() {
    // prefixItems: first 2 are positional, items applies to the rest
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "prefixItems": [
            {"type": "string"},
            {"type": "number"}
        ],
        "items": {"type": "boolean"}
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // First: string, second: number, rest: boolean
    assert!(validator.is_valid(&json!(["name", 3.14, true, false])));
    // Wrong type in prefix
    assert!(!validator.is_valid(&json!([42, 3.14, true])));
    // Wrong type in items (beyond prefix)
    assert!(!validator.is_valid(&json!(["name", 3.14, "not boolean"])));
}

#[test]
fn draft2020_12_prefix_items_only_no_items() {
    // Only prefixItems, no items — extra items allowed
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "prefixItems": [
            {"type": "string"}
        ]
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["hello", 1, 2, 3, "anything"])));
    assert!(!validator.is_valid(&json!([42, "anything"]))); // first not string
}

#[test]
fn draft2020_12_items_only_no_prefix_items() {
    // No prefixItems, items applies to ALL items
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "items": {"type": "string"}
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["a", "b", "c"])));
    assert!(!validator.is_valid(&json!(["a", 42, "c"])));
}

#[test]
fn draft2020_12_items_false_no_items_allowed() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "prefixItems": [
            {"type": "string"}
        ],
        "items": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["hello"])));
    assert!(!validator.is_valid(&json!(["hello", "extra"])));
}

#[test]
fn draft2020_12_items_false_allows_prefix() {
    // items: false means no items beyond prefixItems
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "prefixItems": [
            {"type": "string"},
            {"type": "number"}
        ],
        "items": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["Alice", 30])));
    assert!(!validator.is_valid(&json!(["Alice", 30, true])));
}

// ── contentSchema ──────────────────────────────────────────────────

#[test]
fn content_schema_standalone_valid_json_object() {
    // contentSchema validates the string content as JSON
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentSchema": {
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "required": ["name"]
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // String containing valid JSON object with required "name"
    assert!(validator.is_valid(&json!({"name": "test"})));
}

#[test]
fn content_schema_standalone_invalid_json() {
    // contentSchema is annotation-only in Draft 2020-12 (spec: invalid content passes)
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentSchema": {
            "type": "object"
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // Annotation-only: contentSchema is not validated, string passes type check
    assert!(validator.is_valid(&json!("not json")));
}

#[test]
fn content_schema_standalone_json_wrong_type() {
    // contentSchema is annotation-only in Draft 2020-12
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentSchema": {
            "type": "object",
            "required": ["id"]
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // Annotation-only: contentSchema not validated, string passes type check
    assert!(validator.is_valid(&json!("[1, 2, 3]")));
}

#[test]
fn content_schema_with_base64_encoding() {
    // base64-encoded JSON that must match contentSchema
    // base64 of {"answer":42} = eyJhbnN3ZXIiOjQyfQ==
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentEncoding": "base64",
        "contentMediaType": "application/json",
        "contentSchema": {
            "type": "object",
            "properties": {
                "answer": {"type": "integer"}
            },
            "required": ["answer"]
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!("eyJhbnN3ZXIiOjQyfQ==")));
}

#[test]
fn content_schema_with_base64_wrong_schema() {
    // contentSchema is annotation-only in Draft 2020-12; decoded content is not validated
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentEncoding": "base64",
        "contentMediaType": "application/json",
        "contentSchema": {
            "type": "object",
            "required": ["answer"]
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // Annotation-only: content keywords not validated, string passes
    assert!(validator.is_valid(&json!("eyJ3cm9uZyI6ImZpZWxkIn0=")));
}

#[test]
fn content_schema_non_string_instance_passes() {
    // contentSchema only applies to strings
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentSchema": {"type": "object"}
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(42)));
    assert!(validator.is_valid(&json!([1, 2])));
    assert!(validator.is_valid(&json!(true)));
}

#[test]
fn content_schema_draft2019_09() {
    // contentSchema is annotation-only in Draft 2019-09 (spec: content keywords not validated)
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "contentEncoding": "base64",
        "contentMediaType": "application/json",
        "contentSchema": {
            "type": "array",
            "items": {"type": "integer"}
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft201909)
        .build(&schema)
        .unwrap();

    // Annotation-only: content keywords are not validated, strings pass
    assert!(validator.is_valid(&json!("WzEsMiwzXQ==")));
    assert!(validator.is_valid(&json!("WyJhIiwiYiJd")));
}

#[test]
fn content_schema_with_encoding_only_no_media_type() {
    // contentEncoding without contentMediaType: decode, then treat as string value
    // (no JSON parsing without media type)
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contentEncoding": "base64",
        "contentSchema": {
            "type": "string"
        }
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // base64 of {"key":"value"} = eyJrZXkiOiJ2YWx1ZSJ9
    // Decodes to a string, which matches {"type": "string"}
    assert!(validator.is_valid(&json!("eyJrZXkiOiJ2YWx1ZSJ9")));
}

// ── Error reporting ────────────────────────────────────────────────

#[test]
fn tuple_error_reports_wrong_position() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"},
            {"type": "integer"}
        ],
        "additionalItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    let result = validator.validate(&json!([42, "not int"]));
    assert!(result.is_err());
}

#[test]
fn additional_items_false_error_reports_extra() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": [
            {"type": "string"}
        ],
        "additionalItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();

    let errors: Vec<_> = validator.iter_errors(&json!(["ok", "extra"])).collect();
    assert!(!errors.is_empty());
}

// ── Cross-draft behavior ───────────────────────────────────────────

#[test]
fn tuple_additional_items_absent_means_true_all_drafts() {
    // When additionalItems is absent, it defaults to true (allow any)
    for (draft, schema_uri) in [
        (Draft::Draft4, "http://json-schema.org/draft-04/schema#"),
        (Draft::Draft6, "http://json-schema.org/draft-06/schema#"),
        (Draft::Draft7, "http://json-schema.org/draft-07/schema#"),
        (
            Draft::Draft201909,
            "https://json-schema.org/draft/2019-09/schema",
        ),
    ] {
        let schema = json!({
            "$schema": schema_uri,
            "items": [
                {"type": "string"}
            ]
        });

        let validator = ValidationOptions::new()
            .with_draft(draft)
            .build(&schema)
            .unwrap();

        // Extra items of any type should be allowed
        assert!(
            validator.is_valid(&json!(["hello", 42, true, null])),
            "extra items should pass for {:?}",
            draft
        );
    }
}
