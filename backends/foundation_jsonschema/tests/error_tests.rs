use foundation_jsonschema::{
    Location, ValidationError, ValidationErrorBuilder, ValidationErrorKind,
};
use foundation_jsonschema::{JsonType, JsonTypeSet};

#[test]
fn test_validation_error_kind_display_type() {
    let kind = ValidationErrorKind::InvalidType {
        expected: JsonTypeSet::with(JsonType::Number),
        actual: JsonType::String,
    };
    let msg = format!("{kind}");
    assert!(msg.contains("not of type"));
    assert!(msg.contains("number"));
    assert!(msg.contains("string"));
}

#[test]
fn test_validation_error_kind_display_required() {
    let kind = ValidationErrorKind::Required {
        property: "name".into(),
    };
    let msg = format!("{kind}");
    assert!(msg.contains("name"));
    assert!(msg.contains("required"));
}

#[test]
fn test_validation_error_kind_display_minimum() {
    let kind = ValidationErrorKind::Minimum {
        limit: 18.0,
        actual: 12.0,
        exclusive: false,
    };
    let msg = format!("{kind}");
    assert!(msg.contains("12"));
    assert!(msg.contains("18"));
    assert!(msg.contains("less than"));
}

#[test]
fn test_validation_error_kind_display_false_schema() {
    let kind = ValidationErrorKind::FalseSchema;
    assert_eq!(format!("{kind}"), "false schema always fails validation");
}

#[test]
fn test_validation_error_kind_display_custom() {
    let kind = ValidationErrorKind::Custom {
        message: "my custom error".into(),
    };
    let msg = format!("{kind}");
    assert!(msg.contains("my custom error"));
}

#[test]
fn test_keyword_name() {
    assert_eq!(
        ValidationErrorKind::Required {
            property: "x".into()
        }
        .keyword_name(),
        Some("required")
    );
    assert_eq!(
        ValidationErrorKind::InvalidType {
            expected: JsonTypeSet::new(),
            actual: JsonType::Null
        }
        .keyword_name(),
        Some("type")
    );
    assert_eq!(ValidationErrorKind::FalseSchema.keyword_name(), None);
    assert_eq!(
        ValidationErrorKind::Custom {
            message: String::new()
        }
        .keyword_name(),
        None
    );
}

#[test]
fn test_validation_error_builder() {
    use foundation_jsonschema::to_failure;
    let error = ValidationErrorBuilder::new(Location::new(), Location::new()).build(
        ValidationErrorKind::Required {
            property: "x".into(),
        },
    );
    let failure = to_failure(&error);
    assert!(failure.is_some());
    let failure = failure.unwrap();
    assert_eq!(
        failure.kind,
        ValidationErrorKind::Required {
            property: "x".into()
        }
    );
}
