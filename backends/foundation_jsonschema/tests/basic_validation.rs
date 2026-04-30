//! Integration tests — basic validation scenarios.

use foundation_jsonschema::{is_valid, validate, validator_for, Draft, ValidationOptions};
use serde_json::json;

#[test]
fn simple_string_type() {
    let schema = json!({"type": "string"});
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(!v.is_valid(&json!(42)));
}

#[test]
fn object_with_properties() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer", "minimum": 0}
        },
        "required": ["name"]
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!({"name": "Alice", "age": 30})));
    assert!(v.is_valid(&json!({"name": "Bob"})));
    assert!(!v.is_valid(&json!({"age": 30})));
    assert!(!v.is_valid(&json!({"name": 42})));
}

#[test]
fn array_items() {
    let schema = json!({
        "type": "array",
        "items": {"type": "number"}
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!([1, 2.5, 3])));
    assert!(!v.is_valid(&json!([1, "two", 3])));
}

#[test]
fn all_of() {
    let schema = json!({
        "allOf": [
            {"type": "string"},
            {"minLength": 3}
        ]
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(!v.is_valid(&json!("hi")));
}

#[test]
fn any_of() {
    let schema = json!({
        "anyOf": [
            {"type": "string"},
            {"type": "integer"}
        ]
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(v.is_valid(&json!(42)));
    assert!(!v.is_valid(&json!(true)));
}

#[test]
fn one_of() {
    let schema = json!({
        "oneOf": [
            {"type": "integer", "multipleOf": 5},
            {"type": "integer", "multipleOf": 3}
        ]
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!(10))); // multiple of 5 only
    assert!(v.is_valid(&json!(9))); // multiple of 3 only
    assert!(!v.is_valid(&json!(15))); // multiple of both
}

#[test]
fn not_keyword() {
    let schema = json!({
        "not": {"type": "string"}
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!(42)));
    assert!(!v.is_valid(&json!("hello")));
}

#[test]
fn if_then_else() {
    let schema = json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string"}
        },
        "if": {
            "properties": { "kind": { "const": "foo" } }
        },
        "then": {
            "properties": { "foo_val": {"type": "string"} },
            "required": ["foo_val"]
        },
        "else": {
            "properties": { "bar_val": {"type": "integer"} },
            "required": ["bar_val"]
        }
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!({"kind": "foo", "foo_val": "ok"})));
    assert!(v.is_valid(&json!({"kind": "bar", "bar_val": 42})));
    assert!(!v.is_valid(&json!({"kind": "foo", "bar_val": 42})));
}

#[test]
fn enum_validation() {
    let schema = json!({
        "enum": ["red", "green", "blue"]
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("red")));
    assert!(!v.is_valid(&json!("yellow")));
}

#[test]
fn const_validation() {
    let schema = json!({
        "const": 42
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!(42)));
    assert!(!v.is_valid(&json!(43)));
}

#[test]
fn convenience_is_valid() {
    let schema = json!({"type": "string"});
    assert!(is_valid(&schema, &json!("hello")).unwrap());
    assert!(!is_valid(&schema, &json!(42)).unwrap());
}

#[test]
fn convenience_validate() {
    let schema = json!({"type": "string"});
    assert!(validate(&schema, &json!("hello")).is_ok());
    assert!(validate(&schema, &json!(42)).is_err());
}

#[test]
fn draft_selection() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"}
        }
    });

    // All drafts should accept this simple schema
    let v4 = ValidationOptions::new()
        .with_draft(Draft::Draft4)
        .build(&schema)
        .unwrap();
    let v7 = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();
    let v2020 = ValidationOptions::new()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap();

    let instance = json!({"name": "Alice"});
    assert!(v4.is_valid(&instance));
    assert!(v7.is_valid(&instance));
    assert!(v2020.is_valid(&instance));
}

#[test]
fn pattern_validation() {
    let schema = json!({
        "type": "string",
        "pattern": "^[a-z]+$"
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!("hello")));
    assert!(!v.is_valid(&json!("Hello")));
    assert!(!v.is_valid(&json!("123")));
}

#[test]
fn additional_properties_false() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"}
        },
        "additionalProperties": false
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!({"name": "Alice"})));
    assert!(!v.is_valid(&json!({"name": "Alice", "extra": 1})));
}

#[test]
fn pattern_properties() {
    let schema = json!({
        "patternProperties": {
            "^S_": {"type": "string"},
            "^I_": {"type": "integer"}
        },
        "additionalProperties": false
    });
    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!({"S_name": "Alice", "I_age": 30})));
    assert!(!v.is_valid(&json!({"S_name": 42})));
}

#[test]
fn min_max_constraints() {
    let schema = json!({
        "type": "string",
        "minLength": 2,
        "maxLength": 5
    });
    let v = validator_for(&schema).unwrap();
    assert!(!v.is_valid(&json!("a")));
    assert!(v.is_valid(&json!("ab")));
    assert!(v.is_valid(&json!("hello")));
    assert!(!v.is_valid(&json!("hello!")));
}

#[test]
fn validator_draft_returns_correct() {
    let schema = json!({"type": "string"});
    let v = ValidationOptions::new()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .unwrap();
    assert_eq!(v.draft(), Draft::Draft7);
}
