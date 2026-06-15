//! Unit tests — array schema builder.

use foundation_jsonschema::scheme;
use serde_json::json;

#[test]
fn array_default_schema() {
    let schema = scheme::array().build_schema();
    assert_eq!(schema, json!({"type": "array"}));
}

#[test]
fn array_items() {
    let schema = scheme::array().items(scheme::string()).build_schema();
    assert_eq!(schema["items"]["type"], "string");
}

#[test]
fn array_items_with_constraints() {
    let schema = scheme::array()
        .items(scheme::string().min_len(1).max_len(30))
        .build_schema();
    assert_eq!(schema["items"]["minLength"], 1);
    assert_eq!(schema["items"]["maxLength"], 30);
}

#[test]
fn array_prefix_items() {
    let schema = scheme::array()
        .prefix_items(vec![scheme::number(), scheme::number(), scheme::number()])
        .build_schema();
    let prefix = schema["prefixItems"].as_array().unwrap();
    assert_eq!(prefix.len(), 3);
    assert_eq!(prefix[0]["type"], "number");
    assert_eq!(prefix[1]["type"], "number");
    assert_eq!(prefix[2]["type"], "number");
}

#[test]
fn array_additional_items() {
    let schema = scheme::array()
        .prefix_items(vec![scheme::number(), scheme::number()])
        .additional_items(scheme::boolean())
        .build_schema();
    assert_eq!(schema["additionalItems"]["type"], "boolean");
}

#[test]
fn array_min_items() {
    let schema = scheme::array().min_items(1).build_schema();
    assert_eq!(schema["minItems"], json!(1));
}

#[test]
fn array_max_items() {
    let schema = scheme::array().max_items(10).build_schema();
    assert_eq!(schema["maxItems"], json!(10));
}

#[test]
fn array_len_exact() {
    let schema = scheme::array().len(3).build_schema();
    assert_eq!(schema["minItems"], json!(3));
    assert_eq!(schema["maxItems"], json!(3));
}

#[test]
fn array_unique() {
    let schema = scheme::array().unique().build_schema();
    assert_eq!(schema["uniqueItems"], json!(true));
}

#[test]
fn array_nonempty() {
    let schema = scheme::array().nonempty().build_schema();
    assert_eq!(schema["minItems"], json!(1));
}

#[test]
fn array_contains() {
    let schema = scheme::array()
        .contains(scheme::integer().min(0))
        .build_schema();
    assert_eq!(schema["contains"]["minimum"], json!(0));
}

#[test]
fn array_min_contains() {
    let schema = scheme::array().min_contains(2).build_schema();
    assert_eq!(schema["minContains"], json!(2));
}

#[test]
fn array_max_contains() {
    let schema = scheme::array().max_contains(5).build_schema();
    assert_eq!(schema["maxContains"], json!(5));
}

#[test]
fn array_description_and_default() {
    let schema = scheme::array()
        .description("List of tags")
        .default(json!([]))
        .build_schema();
    assert_eq!(schema["description"], "List of tags");
    assert_eq!(schema["default"], json!([]));
}

#[test]
fn array_nullable() {
    let schema = scheme::array().nullable().build_schema();
    assert_eq!(schema["type"], json!(["array", "null"]));
}

#[test]
fn array_optional() {
    let schema = scheme::array().optional().build_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of[0]["type"], "array");
    assert_eq!(any_of[1]["type"], "null");
}

#[test]
fn array_of_constructor() {
    let schema = scheme::array_of(scheme::string().min_len(1)).build_schema();
    assert_eq!(schema["items"]["type"], "string");
    assert_eq!(schema["items"]["minLength"], 1);
}

#[test]
fn array_combined_constraints() {
    let schema = scheme::array()
        .items(scheme::string().min_len(1).max_len(30))
        .min_items(1)
        .max_items(10)
        .unique()
        .build_schema();

    assert_eq!(schema["type"], "array");
    assert_eq!(schema["items"]["minLength"], 1);
    assert_eq!(schema["items"]["maxLength"], 30);
    assert_eq!(schema["minItems"], 1);
    assert_eq!(schema["maxItems"], 10);
    assert_eq!(schema["uniqueItems"], true);
}

#[test]
fn array_tuple_type() {
    // Tuple: [number, number, number, ...boolean]
    let schema = scheme::array()
        .prefix_items(vec![scheme::number(), scheme::number(), scheme::number()])
        .additional_items(scheme::boolean())
        .build_schema();

    assert_eq!(schema["prefixItems"].as_array().unwrap().len(), 3);
    assert_eq!(schema["additionalItems"]["type"], "boolean");
}

#[test]
fn array_validates_with_compiler() {
    use foundation_jsonschema::validator_for;

    let schema = scheme::array()
        .items(scheme::string().min_len(1))
        .min_items(1)
        .max_items(3)
        .unique()
        .build_schema();

    let v = validator_for(&schema).unwrap();
    assert!(v.is_valid(&json!(["a", "b"])));
    assert!(!v.is_valid(&json!([]))); // min_items: 1
    assert!(!v.is_valid(&json!(["a", "b", "c", "d"]))); // max_items: 3
    assert!(!v.is_valid(&json!(["a", "a"]))); // unique
    assert!(!v.is_valid(&json!(["", "b"]))); // min_len: 1
}

#[test]
fn array_build_schema_is_non_consuming() {
    let builder = scheme::array().items(scheme::string()).unique();
    let schema1 = builder.build_schema();
    let schema2 = builder.build_schema();
    assert_eq!(schema1, schema2);
}
