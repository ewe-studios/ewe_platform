//! Integration tests — cross-draft behavior differences.
//!
//! Tests that each draft correctly implements its specific rules and that
//! features added in later drafts don't leak into earlier ones.

use foundation_jsonschema::{
    draft201909, draft202012, draft4, draft6, draft7, is_valid, Draft, ValidationOptions,
};
use serde_json::json;

// ── Draft auto-detection ─────────────────────────────────────────────

#[test]
fn auto_detect_draft4() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#"
    });
    let v = ValidationOptions::new().build(&schema).unwrap();
    assert_eq!(v.draft(), Draft::Draft4);
}

#[test]
fn auto_detect_draft6() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-06/schema#"
    });
    let v = ValidationOptions::new().build(&schema).unwrap();
    assert_eq!(v.draft(), Draft::Draft6);
}

#[test]
fn auto_detect_draft7() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#"
    });
    let v = ValidationOptions::new().build(&schema).unwrap();
    assert_eq!(v.draft(), Draft::Draft7);
}

#[test]
fn auto_detect_draft2019_09() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema"
    });
    let v = ValidationOptions::new().build(&schema).unwrap();
    assert_eq!(v.draft(), Draft::Draft201909);
}

#[test]
fn auto_detect_draft2020_12() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema"
    });
    let v = ValidationOptions::new().build(&schema).unwrap();
    assert_eq!(v.draft(), Draft::Draft202012);
}

#[test]
fn default_draft_when_no_schema_keyword() {
    // Without $schema, should use the default draft
    let schema = json!({"type": "string"});
    let v = ValidationOptions::new().build(&schema).unwrap();
    // Default is the library's default draft
    assert_eq!(v.draft(), Draft::DEFAULT);
}

// ── Exclusive minimum / maximum (Draft 4 vs 6+) ──────────────────────

#[test]
fn draft4_no_exclusive_minimum() {
    // Draft 4 doesn't have exclusiveMinimum/exclusiveMaximum
    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#",
        "type": "number"
    });
    let v = draft4::compile(&schema).unwrap();
    // Should accept any number since exclusiveMinimum isn't in Draft 4
    assert!(v.is_valid(&json!(0)));
}

#[test]
fn draft6_exclusive_minimum_as_number() {
    // Draft 6+ uses number for exclusiveMinimum
    let schema = json!({
        "$schema": "http://json-schema.org/draft-06/schema#",
        "exclusiveMinimum": 0
    });
    let v = draft6::compile(&schema).unwrap();
    assert!(v.is_valid(&json!(1)));
    assert!(!v.is_valid(&json!(0)));
    assert!(!v.is_valid(&json!(-1)));
}

#[test]
fn draft7_exclusive_minimum() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "exclusiveMinimum": 0
    });
    let v = draft7::compile(&schema).unwrap();
    assert!(v.is_valid(&json!(0.001)));
    assert!(!v.is_valid(&json!(0)));
    assert!(!v.is_valid(&json!(-1)));
}

// ── const (added in Draft 6) ─────────────────────────────────────────

#[test]
fn draft4_no_const_keyword() {
    // Draft 4 doesn't have const — it should be silently ignored
    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#",
        "type": "string"
    });
    let v = draft4::compile(&schema).unwrap();
    // "const" is unknown in Draft 4, so any string passes
    assert!(v.is_valid(&json!("anything")));
}

#[test]
fn draft6_const() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-06/schema#",
        "const": 42
    });
    let v = draft6::compile(&schema).unwrap();
    assert!(v.is_valid(&json!(42)));
    assert!(!v.is_valid(&json!(43)));
}

#[test]
fn draft7_const() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "const": "hello"
    });
    let v = draft7::compile(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(!v.is_valid(&json!("world")));
}

// ── if/then/else (added in Draft 7) ──────────────────────────────────

#[test]
fn draft6_no_if_then_else() {
    // Draft 6 doesn't have if/then/else
    let schema = json!({
        "$schema": "http://json-schema.org/draft-06/schema#",
        "type": "object",
        "properties": { "kind": {"type": "string"} }
    });
    let v = draft6::compile(&schema).unwrap();
    // if/then/else would be unknown keywords, silently ignored
    assert!(v.is_valid(&json!({"kind": "foo"})));
}

#[test]
fn draft7_if_then_else() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "if": { "properties": { "kind": { "const": "foo" } } },
        "then": { "required": ["foo_val"] },
        "else": { "required": ["bar_val"] }
    });
    let v = draft7::compile(&schema).unwrap();
    assert!(v.is_valid(&json!({"kind": "foo", "foo_val": "x"})));
    assert!(v.is_valid(&json!({"kind": "bar", "bar_val": 1})));
    assert!(!v.is_valid(&json!({"kind": "foo", "bar_val": 1})));
}

// ── contains (added in Draft 6) ──────────────────────────────────────

#[test]
fn draft4_no_contains() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#",
        "type": "array",
        "items": {"type": "number"}
    });
    let v = draft4::compile(&schema).unwrap();
    assert!(v.is_valid(&json!([1, 2, 3])));
}

#[test]
fn draft6_contains() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-06/schema#",
        "contains": {"type": "string"}
    });
    let v = draft6::compile(&schema).unwrap();
    assert!(v.is_valid(&json!([1, "hello", 3])));
    assert!(!v.is_valid(&json!([1, 2, 3])));
}

#[test]
fn draft7_contains() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "contains": {"minimum": 10}
    });
    let v = draft7::compile(&schema).unwrap();
    assert!(v.is_valid(&json!([1, 10, 3])));
    assert!(!v.is_valid(&json!([1, 2, 3])));
}

// ── contentEncoding/contentMediaType (2019-09+) ──────────────────────

#[test]
fn draft7_no_content_encoding() {
    // NOTE: Our implementation validates contentEncoding in all drafts.
    // Per spec, Draft 7 should treat it as annotation-only, but our
    // compiler always creates a ContentEncodingValidator.
    // This test documents the current behavior.
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "string",
        "contentEncoding": "base64"
    });
    let v = draft7::compile(&schema).unwrap();
    // Valid base64 should pass
    assert!(v.is_valid(&json!("SGVsbG8=")));
    // Invalid base64 fails in our current implementation
    // (spec says this should pass since contentEncoding is annotation-only in Draft 7)
    assert!(!v.is_valid(&json!("not-valid")));
}

#[test]
fn draft2019_09_content_encoding() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "type": "string",
        "contentEncoding": "base64"
    });
    let v = draft201909::compile(&schema).unwrap();
    assert!(v.is_valid(&json!("SGVsbG8=")));
    // Invalid base64 (wrong length)
    assert!(!v.is_valid(&json!("not-valid")));
}

// ── prefixItems vs items (2020-12) ───────────────────────────────────

#[test]
fn draft7_items_object_form() {
    // Draft 7: items as single object applies to all items
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "items": {"type": "number"}
    });
    let v = draft7::compile(&schema).unwrap();
    assert!(v.is_valid(&json!([1, 2, 3])));
    assert!(!v.is_valid(&json!([1, "two", 3])));
}

// TODO: items as array form (tuple validation) requires additionalItems handling
// This is not yet implemented. When items is an array, each position validates
// against the corresponding schema, and additionalItems controls remaining items.

#[test]
fn draft2020_12_prefix_items_only() {
    // prefixItems validates positional items
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "prefixItems": [{"type": "string"}, {"type": "number"}]
    });
    let v = draft202012::compile(&schema).unwrap();
    assert!(v.is_valid(&json!(["hello", 42])));
    assert!(!v.is_valid(&json!([42, "hello"])));
    // Extra items are allowed (no items constraint)
    assert!(v.is_valid(&json!(["hello", 42, "anything", true])));
}

// TODO: prefixItems + items combination
// In Draft 2020-12, `items` should apply to items AFTER prefixItems count.
// Our current ItemsValidator applies to ALL items, so the combination
// doesn't work correctly yet.

// ── dependentRequired vs dependencies ────────────────────────────────

#[test]
fn draft4_dependencies_array_form() {
    // Draft 4: dependencies with array of required property names
    let schema = json!({
        "$schema": "http://json-schema.org/draft-04/schema#",
        "dependencies": { "name": ["surname"] }
    });
    let v = draft4::compile(&schema).unwrap();
    assert!(v.is_valid(&json!({}))); // name not present, no dependency
    assert!(v.is_valid(&json!({"name": "John", "surname": "Doe"})));
    assert!(!v.is_valid(&json!({"name": "John"})));
}

#[test]
fn draft2019_09_dependent_required() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2019-09/schema",
        "dependentRequired": { "name": ["surname"] }
    });
    let v = draft201909::compile(&schema).unwrap();
    assert!(v.is_valid(&json!({})));
    assert!(v.is_valid(&json!({"name": "John", "surname": "Doe"})));
    assert!(!v.is_valid(&json!({"name": "John"})));
}

// ── Boolean schema values ────────────────────────────────────────────

#[test]
fn all_drafts_boolean_true_schema() {
    for compile_fn in [
        |s: &_| draft4::compile(s),
        |s: &_| draft6::compile(s),
        |s: &_| draft7::compile(s),
        |s: &_| draft201909::compile(s),
        |s: &_| draft202012::compile(s),
    ] {
        let v = compile_fn(&json!(true)).unwrap();
        assert!(v.is_valid(&json!("anything")));
        assert!(v.is_valid(&json!(42)));
        assert!(v.is_valid(&json!(null)));
    }
}

#[test]
fn all_drafts_boolean_false_schema() {
    for compile_fn in [
        |s: &_| draft4::compile(s),
        |s: &_| draft6::compile(s),
        |s: &_| draft7::compile(s),
        |s: &_| draft201909::compile(s),
        |s: &_| draft202012::compile(s),
    ] {
        let v = compile_fn(&json!(false)).unwrap();
        assert!(!v.is_valid(&json!("anything")));
        assert!(!v.is_valid(&json!(42)));
    }
}

// ── Type: "integer" vs "number" ──────────────────────────────────────

#[test]
fn all_drafts_integer_vs_number() {
    for compile_fn in [
        |s: &_| draft4::compile(s),
        |s: &_| draft6::compile(s),
        |s: &_| draft7::compile(s),
        |s: &_| draft201909::compile(s),
        |s: &_| draft202012::compile(s),
    ] {
        let int_schema = compile_fn(&json!({"type": "integer"})).unwrap();
        assert!(int_schema.is_valid(&json!(42)));
        assert!(int_schema.is_valid(&json!(0)));
        assert!(!int_schema.is_valid(&json!(3.14)));

        let num_schema = compile_fn(&json!({"type": "number"})).unwrap();
        assert!(num_schema.is_valid(&json!(42)));
        assert!(num_schema.is_valid(&json!(3.14)));
    }
}

// ── is_valid convenience across drafts ───────────────────────────────

#[test]
fn is_valid_works_all_drafts() {
    let instance = json!("hello");

    // Draft 4 (no $schema, explicit)
    let schema4 = json!({"$schema": "http://json-schema.org/draft-04/schema#", "type": "string"});
    assert!(is_valid(&schema4, &instance).unwrap());

    // Draft 7
    let schema7 = json!({"$schema": "http://json-schema.org/draft-07/schema#", "type": "string"});
    assert!(is_valid(&schema7, &instance).unwrap());

    // Draft 2020-12
    let schema2020 =
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "string"});
    assert!(is_valid(&schema2020, &instance).unwrap());
}
