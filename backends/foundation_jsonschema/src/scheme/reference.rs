//! Reference schema builders — $ref and $dynamicRef.

use super::Value;

/// Create a `$ref` reference to a schema at the given path.
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::ref_("#/$defs/address");
/// // → {"$ref": "#/$defs/address"}
/// ```
#[must_use]
pub fn ref_(path: &str) -> Value {
    Value::Object(
        [("$ref".into(), Value::String(path.into()))]
            .into_iter()
            .collect(),
    )
}

/// Create a `$dynamicRef` reference (Draft 2020-12).
///
/// ```
/// use foundation_jsonschema::scheme;
/// let schema = scheme::dynamic_ref("#node");
/// // → {"$dynamicRef": "#node"}
/// ```
#[must_use]
pub fn dynamic_ref(path: &str) -> Value {
    Value::Object(
        [("$dynamicRef".into(), Value::String(path.into()))]
            .into_iter()
            .collect(),
    )
}
