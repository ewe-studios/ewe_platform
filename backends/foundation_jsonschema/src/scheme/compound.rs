//! Compound schema builders — anyOf, oneOf, allOf, not, if/then/else.

use super::Value;

/// Create an `anyOf` (union) schema from a list of sub-schemas.
///
/// The instance must validate against at least one sub-schema.
#[must_use]
pub fn any_of(schemas: Vec<Value>) -> Value {
    Value::Object(
        [(
            "anyOf".into(),
            Value::Array(schemas),
        )]
        .into_iter()
        .collect(),
    )
}

/// Alias for `any_of`. Creates a union schema.
#[must_use]
pub fn union(schemas: Vec<Value>) -> Value {
    any_of(schemas)
}

/// Create a `oneOf` schema. The instance must validate against exactly one sub-schema.
#[must_use]
pub fn one_of(schemas: Vec<Value>) -> Value {
    Value::Object(
        [(
            "oneOf".into(),
            Value::Array(schemas),
        )]
        .into_iter()
        .collect(),
    )
}

/// Create an `allOf` (intersection) schema. The instance must validate against all sub-schemas.
#[must_use]
pub fn all_of(schemas: Vec<Value>) -> Value {
    Value::Object(
        [(
            "allOf".into(),
            Value::Array(schemas),
        )]
        .into_iter()
        .collect(),
    )
}

/// Create an intersection of two schemas (shorthand for `all_of`).
#[must_use]
pub fn intersection(left: Value, right: Value) -> Value {
    all_of(vec![left, right])
}

/// Create a `not` schema. The instance must NOT validate against the sub-schema.
#[must_use]
pub fn not(schema: impl Into<Value>) -> Value {
    Value::Object(
        [("not".into(), schema.into())]
            .into_iter()
            .collect(),
    )
}

/// Create an `if/then` conditional schema.
#[must_use]
pub fn if_then(if_schema: Value, then_schema: Value) -> Value {
    Value::Object(
        [
            ("if".into(), if_schema),
            ("then".into(), then_schema),
        ]
        .into_iter()
        .collect(),
    )
}

/// Create an `if/then/else` conditional schema.
#[must_use]
pub fn if_then_else(
    if_schema: Value,
    then_schema: Value,
    else_schema: Value,
) -> Value {
    Value::Object(
        [
            ("if".into(), if_schema),
            ("then".into(), then_schema),
            ("else".into(), else_schema),
        ]
        .into_iter()
        .collect(),
    )
}
