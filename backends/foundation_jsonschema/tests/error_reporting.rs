//! Integration tests — error reporting.

use foundation_jsonschema::ValidationErrorKind;
use serde_json::json;

#[test]
fn type_error_kind() {
    let schema = json!({"type": "string"});
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!(42)).collect();
    assert!(!errors.is_empty());
    assert!(matches!(errors[0].current_context(), ValidationErrorKind::InvalidType { .. }));
}

#[test]
fn required_error_kind() {
    let schema = json!({
        "type": "object",
        "required": ["name", "age"]
    });
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!({})).collect();
    assert!(errors.len() >= 2);
    assert!(errors.iter().all(|e| matches!(e.current_context(), ValidationErrorKind::Required { .. })));
}

#[test]
fn min_length_error() {
    let schema = json!({"type": "string", "minLength": 5});
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!("hi")).collect();
    assert!(!errors.is_empty());
    match errors[0].current_context() {
        ValidationErrorKind::MinLength { min, actual } => {
            assert_eq!(*min, 5);
            assert_eq!(*actual, 2);
        }
        _ => panic!("expected MinLength error"),
    }
}

#[test]
fn max_items_error() {
    let schema = json!({"type": "array", "maxItems": 2});
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!([1, 2, 3])).collect();
    assert!(!errors.is_empty());
    match errors[0].current_context() {
        ValidationErrorKind::MaxItems { max, actual } => {
            assert_eq!(*max, 2);
            assert_eq!(*actual, 3);
        }
        _ => panic!("expected MaxItems error"),
    }
}

#[test]
fn minimum_error() {
    let schema = json!({"type": "number", "minimum": 0.0});
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!(-5)).collect();
    assert!(!errors.is_empty());
    match errors[0].current_context() {
        ValidationErrorKind::Minimum { limit, actual, .. } => {
            assert_eq!(*limit, 0.0);
            assert_eq!(*actual, -5.0);
        }
        _ => panic!("expected Minimum error"),
    }
}

#[test]
fn is_valid_fast_path() {
    let schema = json!({"type": "object", "required": ["a"]});
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    assert!(!v.is_valid(&json!({})));
    assert!(v.is_valid(&json!({"a": 1})));
}

#[test]
fn validate_first_error() {
    let schema = json!({
        "type": "object",
        "required": ["a", "b", "c"]
    });
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let result = v.validate(&json!({}));
    assert!(result.is_err());
    // validate() returns only the first error
    let err = result.unwrap_err();
    assert!(matches!(err.current_context(), ValidationErrorKind::Required { .. }));
}

#[test]
fn one_of_multiple_valid_error() {
    let schema = json!({
        "oneOf": [
            {"type": "integer"},
            {"type": "number"}
        ]
    });
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!(42)).collect();
    // 42 matches both int and number in oneOf
    assert!(!errors.is_empty());
    assert!(matches!(errors[0].current_context(), ValidationErrorKind::OneOfMultipleValid));
}

#[test]
fn all_of_partial_fail() {
    let schema = json!({
        "allOf": [
            {"type": "string"},
            {"minLength": 10}
        ]
    });
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!("short")).collect();
    assert!(!errors.is_empty());
}

#[test]
fn error_display() {
    let schema = json!({"type": "string"});
    let v = foundation_jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = v.iter_errors(&json!(42)).collect();
    // Error should have a display representation
    let display = format!("{}", errors[0]);
    assert!(!display.is_empty());
}
