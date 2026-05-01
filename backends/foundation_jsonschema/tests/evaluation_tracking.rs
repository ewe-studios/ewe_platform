//! Integration tests — evaluation tracking, unevaluated*, contains, and if/then/else.
//!
//! Tests for:
//! - Combiners (allOf/anyOf/oneOf) propagating evaluation marks
//! - Contains with minContains/maxContains
//! - Contains marking matched items for unevaluatedItems
//! - If/then/else propagating evaluation marks
//! - $ref evaluation propagation
//! - UnevaluatedProperties/UnevaluatedItems with various combinations

use foundation_jsonschema::{Draft, ValidationOptions};
use serde_json::json;

// ── Combiners with unevaluatedProperties ─────────────────────────────

#[test]
fn all_of_unevaluated_properties() {
    // allOf should propagate evaluation marks from its sub-schemas
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "allOf": [
            {"properties": {"name": {"type": "string"}}},
            {"properties": {"age": {"type": "integer"}}}
        ],
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // Both properties defined in allOf branches should be evaluated
    assert!(validator.is_valid(&json!({"name": "Alice", "age": 30})));
    // Extra property should fail
    assert!(!validator.is_valid(&json!({"name": "Alice", "age": 30, "extra": true})));
}

#[test]
fn any_of_unevaluated_properties() {
    // anyOf should propagate evaluation marks from the matching branch.
    // Note: anyOf stops at the first matching branch. Since `properties` only
    // validates properties that exist, the first branch always "passes" (it has
    // no required properties). To test proper evaluation, we use schemas where
    // only one branch can match.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "anyOf": [
            {
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            },
            {
                "properties": {"id": {"type": "integer"}},
                "required": ["id"]
            }
        ],
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // "name" matches first branch (required), "name" evaluated
    assert!(validator.is_valid(&json!({"name": "Alice"})));
    // "id" fails first branch (no "name"), matches second branch
    assert!(validator.is_valid(&json!({"id": 42})));
    // Extra property should fail
    assert!(!validator.is_valid(&json!({"name": "Alice", "extra": true})));
}

#[test]
fn one_of_unevaluated_properties() {
    // oneOf should propagate evaluation marks from the single matching branch.
    // Using required to ensure only one branch can match.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "oneOf": [
            {
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            },
            {
                "properties": {"id": {"type": "integer"}},
                "required": ["id"]
            }
        ],
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"name": "Alice"})));
    assert!(validator.is_valid(&json!({"id": 42})));
    // Both matching would be 2 — should fail oneOf
    assert!(!validator.is_valid(&json!({"name": "Alice", "id": 42})));
}

// ── Combiners with unevaluatedItems ──────────────────────────────────

#[test]
fn all_of_unevaluated_items() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "allOf": [
            {"prefixItems": [{"type": "string"}]},
            {"prefixItems": [{"type": "string"}, {"type": "number"}]}
        ],
        "unevaluatedItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["Alice", 30])));
    assert!(!validator.is_valid(&json!(["Alice", 30, "extra"])));
}

// ── Contains with minContains/maxContains ────────────────────────────

#[test]
fn min_contains_draft2019_09() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "contains": {"type": "integer"},
        "minContains": 2
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft201909)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!([1, 2, "a", "b"]))); // 2 integers
    assert!(!validator.is_valid(&json!([1, "a", "b"]))); // only 1 integer
    assert!(!validator.is_valid(&json!(["a", "b"]))); // 0 integers
}

#[test]
fn max_contains_draft2019_09() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "contains": {"type": "integer"},
        "maxContains": 2
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft201909)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!([1, 2, "a"]))); // 2 integers
    assert!(!validator.is_valid(&json!([1, 2, 3, "a"]))); // 3 integers > max
}

#[test]
fn min_and_max_contains_combined() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contains": {"type": "string"},
        "minContains": 1,
        "maxContains": 3
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["a", "b"]))); // 2 strings
    assert!(!validator.is_valid(&json!([1, 2, 3]))); // 0 strings
    assert!(!validator.is_valid(&json!(["a", "b", "c", "d"]))); // 4 > max
}

#[test]
fn contains_marks_matched_items_for_unevaluated() {
    // In Draft 2020-12, `contains` DOES mark matched items as evaluated for
    // unevaluatedItems. Only items that match the contains schema are marked.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contains": {"type": "string"},
        "unevaluatedItems": {"type": "integer"}
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // "a" matched by contains (marked evaluated), 42 is unevaluated —
    // 42 passes unevaluatedItems check (it is an integer)
    assert!(validator.is_valid(&json!(["a", 42])));
    // All strings — all matched by contains, all marked evaluated
    assert!(validator.is_valid(&json!(["a", "b"])));
    // No strings — contains fails (requires at least one match)
    assert!(!validator.is_valid(&json!([1, 2, 3])));
    // Only integers — contains fails (no string matches)
    assert!(!validator.is_valid(&json!([1, 2])));
}

// ── If/then/else with unevaluatedProperties ──────────────────────────

#[test]
fn if_then_unevaluated_properties() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "if": {"properties": {"kind": {"const": "person"}}},
        "then": {"properties": {"name": {"type": "string"}, "age": {"type": "integer"}}},
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // kind matches if, then validates name and age — all evaluated
    assert!(validator.is_valid(&json!({"kind": "person", "name": "Alice", "age": 30})));
    // Extra property should fail unevaluatedProperties
    assert!(!validator.is_valid(&json!({"kind": "person", "name": "Alice", "extra": true})));
    // kind doesn't match if — if marks discarded, no else, "kind" unevaluated
    assert!(!validator.is_valid(&json!({"kind": "other"})));
    // kind doesn't match, extra property — both unevaluated
    assert!(!validator.is_valid(&json!({"kind": "other", "extra": true})));
}

#[test]
fn if_else_unevaluated_properties() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "if": {"properties": {"kind": {"const": "admin"}}, "required": ["kind"]},
        "else": {
            "properties": {"role": {"const": "user"}},
            "required": ["role"]
        },
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // kind=admin — if passes, else not applied, only "kind" evaluated
    assert!(validator.is_valid(&json!({"kind": "admin"})));
    // kind=user — if fails (not admin), its marks discarded, else applies "role"
    // "kind" is unevaluated (if marks discarded) — fails unevaluatedProperties
    assert!(!validator.is_valid(&json!({"kind": "user", "role": "user"})));
    // kind=other — if fails, else requires "role" which is missing
    assert!(!validator.is_valid(&json!({"kind": "other"})));
}

// ── $ref evaluation propagation ──────────────────────────────────────

#[test]
fn ref_evaluates_properties_for_unevaluated() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {
            "person": {
                "properties": {
                    "name": {"type": "string"},
                    "age": {"type": "integer"}
                }
            }
        },
        "$ref": "#/$defs/person",
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"name": "Alice", "age": 30})));
    assert!(!validator.is_valid(&json!({"name": "Alice", "age": 30, "extra": true})));
}

#[test]
fn ref_evaluates_items_for_unevaluated() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {
            "tuple": {
                "prefixItems": [
                    {"type": "string"},
                    {"type": "number"}
                ]
            }
        },
        "$ref": "#/$defs/tuple",
        "unevaluatedItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!(["Alice", 3.14])));
    assert!(!validator.is_valid(&json!(["Alice", 3.14, "extra"])));
}

// ── Nested combiners with unevaluated ────────────────────────────────

#[test]
fn nested_all_of_unevaluated_properties() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "allOf": [
            {
                "allOf": [
                    {"properties": {"name": {"type": "string"}}}
                ]
            },
            {"properties": {"age": {"type": "integer"}}}
        ],
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"name": "Alice", "age": 30})));
    assert!(!validator.is_valid(&json!({"name": "Alice", "age": 30, "extra": true})));
}

// ── patternProperties with unevaluatedProperties ─────────────────────

#[test]
fn pattern_properties_evaluates_for_unevaluated() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "patternProperties": {
            "^x-": {"type": "string"}
        },
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!({"x-name": "Alice", "x-age": "30"})));
    // "name" doesn't match pattern — unevaluated, should fail
    assert!(!validator.is_valid(&json!({"x-name": "Alice", "name": "Alice"})));
}

// ── additionalProperties with unevaluatedProperties ──────────────────

#[test]
fn additional_properties_evaluates_for_unevaluated() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "properties": {"name": {"type": "string"}},
        "additionalProperties": {"type": "integer"},
        "unevaluatedProperties": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // "name" evaluated by properties, "age" evaluated by additionalProperties
    assert!(validator.is_valid(&json!({"name": "Alice", "age": 30})));
    assert!(!validator.is_valid(&json!({"name": "Alice", "extra": "not int"})));
}

// ── Error reporting for contains ─────────────────────────────────────

#[test]
fn min_contains_error_reporting() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contains": {"type": "integer"},
        "minContains": 3
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    let result = validator.validate(&json!([1, "a", "b"]));
    assert!(result.is_err());
}

#[test]
fn max_contains_error_reporting() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "contains": {"type": "integer"},
        "maxContains": 1
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    let result = validator.validate(&json!([1, 2, 3]));
    assert!(result.is_err());
}

// ── Edge cases ───────────────────────────────────────────────────────

#[test]
fn contains_zero_min_means_always_passes() {
    // minContains: 0 means even empty array passes
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "contains": {"type": "integer"},
        "minContains": 0
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft201909)
        .build(&schema)
        .unwrap();

    assert!(validator.is_valid(&json!([])));
    assert!(validator.is_valid(&json!(["a", "b"])));
}

#[test]
fn unevaluated_items_with_contains_and_prefix_items() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "prefixItems": [{"type": "string"}],
        "contains": {"type": "number"},
        "unevaluatedItems": false
    });

    let validator = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    // Item 0 ("name") evaluated by prefixItems. Item 1 (42) matched by contains
    // (type: number) and marked as evaluated — all items evaluated, passes.
    assert!(validator.is_valid(&json!(["name", 42])));
    // No number present — contains fails (requires at least one match)
    assert!(!validator.is_valid(&json!(["name"])));
}
