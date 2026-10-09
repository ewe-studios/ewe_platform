//! Unit tests — object schema builder.

extern crate alloc;

use alloc::collections::BTreeMap;
use foundation_jsonschema::scheme;
use serde_json::json;

#[test]
fn object_default_schema() {
    let schema = scheme::object().build_schema();
    assert_eq!(schema, json!({"type": "object"}));
}

#[test]
fn object_required_property() {
    let schema = scheme::object()
        .required("name", scheme::string())
        .build_schema();
    assert_eq!(schema["required"], json!(["name"]));
    assert_eq!(schema["properties"]["name"]["type"], "string");
}

#[test]
fn object_optional_property() {
    let schema = scheme::object()
        .optional("bio", scheme::string())
        .build_schema();
    assert_eq!(schema["properties"]["bio"]["type"], "string");
    assert!(schema
        .get("required")
        .is_none_or(|r| r.as_array().is_none_or(|a| !a.iter().any(|v| v == "bio"))));
}

#[test]
fn object_required_and_optional() {
    let schema = scheme::object()
        .required("name", scheme::string())
        .optional("email", scheme::string().email())
        .build_schema();
    assert_eq!(schema["required"], json!(["name"]));
    assert_eq!(schema["properties"]["name"]["type"], "string");
    assert_eq!(schema["properties"]["email"]["format"], "email");
}

#[test]
fn object_multiple_required() {
    let schema = scheme::object()
        .required("first", scheme::string())
        .required("last", scheme::string())
        .required("age", scheme::integer().min(0))
        .build_schema();
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 3);
    assert!(required.iter().any(|v| v == "first"));
    assert!(required.iter().any(|v| v == "last"));
    assert!(required.iter().any(|v| v == "age"));
}

#[test]
fn object_strict() {
    let schema = scheme::object().strict().build_schema();
    assert_eq!(schema["additionalProperties"], json!(false));
}

#[test]
fn object_loose() {
    let schema = scheme::object().loose().build_schema();
    assert_eq!(schema["additionalProperties"], json!(true));
}

#[test]
fn object_catchall_schema() {
    let schema = scheme::object().catchall(scheme::string()).build_schema();
    assert_eq!(schema["additionalProperties"]["type"], "string");
}

#[test]
fn object_pattern_property() {
    let schema = scheme::object()
        .pattern_property(r"^x-", scheme::string())
        .build_schema();
    assert_eq!(schema["patternProperties"]["^x-"]["type"], "string");
}

#[test]
fn object_min_max_properties() {
    let schema = scheme::object()
        .min_properties(1)
        .max_properties(10)
        .build_schema();
    assert_eq!(schema["minProperties"], json!(1));
    assert_eq!(schema["maxProperties"], json!(10));
}

#[test]
fn object_properties_bulk() {
    let mut map = BTreeMap::new();
    map.insert("a".into(), json!({"type": "string"}));
    map.insert("b".into(), json!({"type": "integer"}));
    let schema = scheme::object().properties(map).build_schema();
    assert_eq!(schema["properties"]["a"]["type"], "string");
    assert_eq!(schema["properties"]["b"]["type"], "integer");
}

#[test]
fn object_description_and_default() {
    let schema = scheme::object()
        .description("A user profile")
        .default(json!({"name": "anon"}))
        .build_schema();
    assert_eq!(schema["description"], "A user profile");
    assert_eq!(schema["default"]["name"], "anon");
}

#[test]
fn object_nullable() {
    let schema = scheme::object().nullable().build_schema();
    assert_eq!(schema["type"], json!(["object", "null"]));
}

#[test]
fn object_optional_value() {
    let schema = scheme::object()
        .required("id", scheme::integer())
        .optional_value()
        .build_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2);
    assert_eq!(any_of[0]["type"], "object");
    assert_eq!(any_of[1]["type"], "null");
}

#[test]
fn object_nested_with_raw_value() {
    let schema = scheme::object()
        .required("address", json!({"type": "object"}))
        .build_schema();
    assert_eq!(schema["properties"]["address"]["type"], "object");
}

#[test]
fn object_nested_builders() {
    let schema = scheme::object()
        .required(
            "user",
            scheme::object()
                .required("name", scheme::string())
                .optional("age", scheme::integer().min(0))
                .strict(),
        )
        .build_schema();
    let user_props = &schema["properties"]["user"]["properties"];
    assert_eq!(user_props["name"]["type"], "string");
    assert_eq!(user_props["age"]["minimum"], 0);
    assert_eq!(schema["properties"]["user"]["additionalProperties"], false);
}

#[test]
fn object_required_passes_builder_directly() {
    // The From<StringSchema> impl lets us pass the builder directly
    let schema = scheme::object()
        .required("name", scheme::string().min_len(1))
        .build_schema();
    assert_eq!(schema["properties"]["name"]["minLength"], 1);
}

#[test]
fn object_strict_with_nested_object_validates() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::object()
        .required("name", scheme::string())
        .strict()
        .build_schema();

    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!({"name": "Alice"})));
    assert!(!v.is_valid(&json!({"name": "Alice", "extra": true})));
}

#[test]
fn object_build_schema_is_non_consuming() {
    let builder = scheme::object().required("name", scheme::string()).strict();
    let schema1 = builder.build_schema();
    let schema2 = builder.build_schema();
    assert_eq!(schema1, schema2);
}
