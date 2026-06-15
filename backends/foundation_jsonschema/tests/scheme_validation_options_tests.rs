//! Unit tests — ValidationOptions schema accessor methods.

use foundation_jsonschema::scheme;
use serde_json::json;

// ── schema() accessor ──────────────────────────────────────────

#[test]
fn schema_returns_reference() {
    let opts = scheme::object().required("name", scheme::string()).build();

    let schema = opts.schema();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["required"], json!(["name"]));
}

#[test]
fn schema_preserves_constraints() {
    let opts = scheme::string().min_len(1).max_len(255).email().build();

    let schema = opts.schema();
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["minLength"], 1);
    assert_eq!(schema["maxLength"], 255);
    assert_eq!(schema["format"], "email");
}

// ── into_schema() accessor ─────────────────────────────────────

#[test]
fn into_schema_takes_ownership() {
    let opts = scheme::integer().min(0).max(100).build();

    let schema = opts.into_schema();
    assert_eq!(schema["type"], "integer");
    assert_eq!(schema["minimum"], 0);
    assert_eq!(schema["maximum"], 100);
}

#[test]
fn into_schema_nested_object() {
    let opts = scheme::object()
        .required(
            "user",
            scheme::object()
                .required("id", scheme::integer())
                .required("name", scheme::string()),
        )
        .build();

    let schema = opts.into_schema();
    assert_eq!(
        schema["properties"]["user"]["properties"]["id"]["type"],
        "integer"
    );
    assert_eq!(
        schema["properties"]["user"]["properties"]["name"]["type"],
        "string"
    );
}

// ── clone_schema() accessor ────────────────────────────────────

#[test]
fn clone_schema_returns_clone() {
    let opts = scheme::array().items(scheme::string()).min_items(1).build();

    let cloned = opts.clone_schema();
    assert_eq!(cloned["type"], "array");
    assert_eq!(cloned["minItems"], 1);
}

#[test]
fn clone_schema_does_not_consume() {
    let opts = scheme::boolean().build();

    let _c1 = opts.clone_schema();
    let _c2 = opts.clone_schema();
    // opts is still available — can compile after cloning
    let validator = opts.compile();
    assert!(validator.is_ok());
}

// ── compile() with embedded schema ─────────────────────────────

#[test]
fn compile_embedded_schema_basic() {
    let validator = scheme::object()
        .required("id", scheme::integer())
        .required("name", scheme::string())
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"id": 1, "name": "test"})));
    assert!(!validator.is_valid(&json!({"id": 1})));
}

#[test]
fn compile_embedded_schema_with_assert_format() {
    let validator = scheme::object()
        .required("email", scheme::string().email())
        .build()
        .assert_format(true)
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"email": "test@example.com"})));
    assert!(!validator.is_valid(&json!({"email": "not-an-email"})));
}

#[test]
fn compile_embedded_schema_with_draft() {
    use foundation_jsonschema::Draft;

    let validator = scheme::object()
        .required("value", scheme::number())
        .build()
        .with_draft(Draft::Draft7)
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!({"value": 42.5})));
}

// ── schema() after optional/nullable wrapping ──────────────────

#[test]
fn schema_optional_wrapped_in_anyof() {
    let opts = scheme::string().optional().build();
    let schema = opts.schema();

    assert!(schema.get("anyOf").is_some());
    assert!(schema.get("type").is_none());
}

#[test]
fn schema_nullable_changes_type_to_array() {
    let opts = scheme::integer().nullable().build();
    let schema = opts.schema();

    assert_eq!(schema["type"], json!(["integer", "null"]));
}

// ── compile chain from primitives ──────────────────────────────

#[test]
fn primitive_compile_chain_string() {
    let validator = scheme::string()
        .min_len(3)
        .max_len(10)
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!("hello")));
    assert!(!validator.is_valid(&json!("ab")));
    assert!(!validator.is_valid(&json!("this is too long")));
}

#[test]
fn primitive_compile_chain_integer() {
    let validator = scheme::integer().min(1).max(100).build().compile().unwrap();

    assert!(validator.is_valid(&json!(50)));
    assert!(!validator.is_valid(&json!(0)));
    assert!(!validator.is_valid(&json!(101)));
}

#[test]
fn primitive_compile_chain_array() {
    let validator = scheme::array()
        .items(scheme::integer())
        .min_items(1)
        .max_items(3)
        .build()
        .compile()
        .unwrap();

    assert!(validator.is_valid(&json!([1, 2])));
    assert!(!validator.is_valid(&json!([])));
    assert!(!validator.is_valid(&json!([1, 2, 3, 4])));
}
