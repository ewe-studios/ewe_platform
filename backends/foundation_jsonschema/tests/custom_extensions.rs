//! Integration tests — custom extensions (formats and keywords).

use foundation_jsonschema::formats::FormatChecker;
use foundation_jsonschema::{
    ErrorIterator, KeywordFactory, LazyLocation, Location, Validate, ValidationContext,
    ValidationError, ValidationErrorBuilder, ValidationErrorKind, ValidationOptions,
};
use serde_json::json;

#[test]
fn custom_format_checker() {
    struct HexColorFormat;

    impl FormatChecker for HexColorFormat {
        fn check(&self, value: &str) -> bool {
            value.len() == 7
                && value.starts_with('#')
                && value[1..].chars().all(|c| c.is_ascii_hexdigit())
        }

        fn format_name(&self) -> &str {
            "hex-color"
        }

        fn clone_box(&self) -> Box<dyn FormatChecker> {
            Box::new(HexColorFormat)
        }
    }

    let schema = json!({
        "type": "string",
        "format": "hex-color"
    });

    let v = ValidationOptions::new()
        .assert_format(true)
        .with_format("hex-color", Box::new(HexColorFormat))
        .build(&schema)
        .unwrap();

    assert!(v.is_valid(&json!("#ff0000")));
    assert!(!v.is_valid(&json!("red")));
    assert!(!v.is_valid(&json!("#gg0000")));
}

#[test]
fn unknown_format_passes() {
    // Unknown formats should always pass validation (per JSON Schema spec)
    let schema = json!({
        "type": "string",
        "format": "unknown-fmt"
    });

    let v = ValidationOptions::new()
        .assert_format(true)
        .build(&schema)
        .unwrap();

    assert!(v.is_valid(&json!("anything")));
}

#[test]
fn format_annotation_only_by_default() {
    // By default, format is annotation-only and doesn't fail
    let schema = json!({
        "type": "string",
        "format": "email"
    });

    let v = ValidationOptions::new().build(&schema).unwrap();

    // Even with an invalid email, it should pass because format is annotation-only
    assert!(v.is_valid(&json!("not-an-email")));
}

#[test]
fn custom_keyword_factory() {
    struct PositiveNumberValidator;

    impl Validate for PositiveNumberValidator {
        fn is_valid(&self, instance: &serde_json::Value, _ctx: &mut ValidationContext) -> bool {
            instance.as_f64().map_or(false, |n| n > 0.0)
        }

        fn validate(
            &self,
            instance: &serde_json::Value,
            instance_path: &LazyLocation<'_>,
            _ctx: &mut ValidationContext,
        ) -> Result<(), ValidationError> {
            if instance.as_f64().map_or(false, |n| n > 0.0) {
                Ok(())
            } else {
                Err(
                    ValidationErrorBuilder::new(instance_path.materialize(), Location::new())
                        .build(ValidationErrorKind::Custom {
                            message: "value must be positive".into(),
                        }),
                )
            }
        }

        fn iter_errors(
            &self,
            instance: &serde_json::Value,
            instance_path: &LazyLocation<'_>,
            ctx: &mut ValidationContext,
        ) -> ErrorIterator {
            match self.validate(instance, instance_path, ctx) {
                Ok(()) => Box::new(core::iter::empty()),
                Err(e) => Box::new(core::iter::once(e)),
            }
        }
    }

    struct PositiveFactory;

    impl KeywordFactory for PositiveFactory {
        fn compile(
            &self,
            _value: &serde_json::Value,
            _schema_path: Location,
        ) -> Result<Box<dyn Validate>, ValidationError> {
            Ok(Box::new(PositiveNumberValidator))
        }

        fn clone_box(&self) -> Box<dyn KeywordFactory> {
            Box::new(PositiveFactory)
        }
    }

    let schema = json!({
        "positive": true
    });

    let v = ValidationOptions::new()
        .with_keyword("positive", Box::new(PositiveFactory))
        .build(&schema)
        .unwrap();

    assert!(v.is_valid(&json!(42)));
    assert!(v.is_valid(&json!(0.1)));
    assert!(!v.is_valid(&json!(0)));
    assert!(!v.is_valid(&json!(-1)));
    assert!(!v.is_valid(&json!("not a number")));
}

#[test]
fn built_in_format_validators() {
    // Test built-in format checkers with assert_format
    let schema = json!({
        "type": "string",
        "format": "ipv4"
    });

    let v = ValidationOptions::new()
        .assert_format(true)
        .build(&schema)
        .unwrap();

    assert!(v.is_valid(&json!("192.168.1.1")));
    assert!(!v.is_valid(&json!("999.999.999.999")));
}

#[test]
fn custom_formats_override_builtin() {
    // Custom format with same name as built-in should take precedence
    struct AlwaysTrueEmail;

    impl FormatChecker for AlwaysTrueEmail {
        fn check(&self, _value: &str) -> bool {
            true
        }

        fn format_name(&self) -> &str {
            "email"
        }

        fn clone_box(&self) -> Box<dyn FormatChecker> {
            Box::new(AlwaysTrueEmail)
        }
    }

    let schema = json!({
        "type": "string",
        "format": "email"
    });

    let v = ValidationOptions::new()
        .assert_format(true)
        .with_format("email", Box::new(AlwaysTrueEmail))
        .build(&schema)
        .unwrap();

    // Custom checker always returns true
    assert!(v.is_valid(&json!("anything")));
}
