//! Literal and enum schema builders — const and enum values.

use super::Value;

/// Create a literal schema that matches only the given value (`"const"`).
///
/// ```
/// use foundation_jsonschema::scheme;
/// use serde_json::json;
/// let schema = scheme::literal(json!("hello"));
/// // → {"const": "hello"}
/// ```
#[must_use]
pub fn literal(value: Value) -> Value {
    Value::Object([("const".into(), value)].into_iter().collect())
}

/// Create an enum schema that matches one of the given values (`"enum"`).
///
/// ```
/// use foundation_jsonschema::scheme;
/// use serde_json::json;
/// let schema = scheme::r#enum(vec![json!("red"), json!("green"), json!("blue")]);
/// // → {"enum": ["red", "green", "blue"]}
/// ```
#[must_use]
pub fn r#enum(values: Vec<Value>) -> Value {
    Value::Object(
        [("enum".into(), Value::Array(values))]
            .into_iter()
            .collect(),
    )
}
