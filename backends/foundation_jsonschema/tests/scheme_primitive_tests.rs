//! Unit tests — primitive schema builders (string, integer, number, boolean, null).

use foundation_jsonschema::scheme;
use serde_json::json;

// ── String Schema ──────────────────────────────────────────────

#[test]
fn string_default_schema() {
    let schema = scheme::string().build_schema();
    assert_eq!(schema, json!({"type": "string"}));
}

#[test]
fn string_min_len() {
    let schema = scheme::string().min_len(1).build_schema();
    assert_eq!(schema["minLength"], json!(1));
}

#[test]
fn string_max_len() {
    let schema = scheme::string().max_len(255).build_schema();
    assert_eq!(schema["maxLength"], json!(255));
}

#[test]
fn string_len_exact() {
    let schema = scheme::string().len(10).build_schema();
    assert_eq!(schema["minLength"], json!(10));
    assert_eq!(schema["maxLength"], json!(10));
}

#[test]
fn string_pattern() {
    let schema = scheme::string().pattern(r"^[a-z]+$").build_schema();
    assert_eq!(schema["pattern"], json!("^[a-z]+$"));
}

#[test]
fn string_format() {
    let schema = scheme::string().format("uri").build_schema();
    assert_eq!(schema["format"], json!("uri"));
}

#[test]
fn string_format_shorthands() {
    assert_eq!(scheme::string().email().build_schema()["format"], "email");
    assert_eq!(scheme::string().uri().build_schema()["format"], "uri");
    assert_eq!(scheme::string().uuid().build_schema()["format"], "uuid");
    assert_eq!(scheme::string().date_time().build_schema()["format"], "date-time");
    assert_eq!(scheme::string().hostname().build_schema()["format"], "hostname");
    assert_eq!(scheme::string().ipv4().build_schema()["format"], "ipv4");
    assert_eq!(scheme::string().ipv6().build_schema()["format"], "ipv6");
    assert_eq!(scheme::string().base64().build_schema()["format"], "base64");
}

#[test]
fn string_enum_values() {
    let schema = scheme::string()
        .enum_values(vec!["a".into(), "b".into(), "c".into()])
        .build_schema();
    assert_eq!(schema["enum"], json!(["a", "b", "c"]));
}

#[test]
fn string_description() {
    let schema = scheme::string().description("A user's name").build_schema();
    assert_eq!(schema["description"], "A user's name");
}

#[test]
fn string_default_value() {
    let schema = scheme::string().default(json!("guest")).build_schema();
    assert_eq!(schema["default"], "guest");
}

#[test]
fn string_nullable() {
    let schema = scheme::string().nullable().build_schema();
    assert_eq!(schema["type"], json!(["string", "null"]));
}

#[test]
fn string_optional() {
    let schema = scheme::string().optional().build_schema();
    assert!(schema["anyOf"].is_array());
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2);
    assert_eq!(any_of[0]["type"], "string");
    assert_eq!(any_of[1]["type"], "null");
}

#[test]
fn string_combined_constraints() {
    let schema = scheme::string()
        .min_len(1)
        .max_len(255)
        .email()
        .description("User email")
        .build_schema();
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["minLength"], 1);
    assert_eq!(schema["maxLength"], 255);
    assert_eq!(schema["format"], "email");
    assert_eq!(schema["description"], "User email");
}

// ── Integer Schema ──────────────────────────────────────────────

#[test]
fn integer_default_schema() {
    let schema = scheme::integer().build_schema();
    assert_eq!(schema, json!({"type": "integer"}));
}

#[test]
fn integer_min_max() {
    let schema = scheme::integer().min(0).max(100).build_schema();
    assert_eq!(schema["minimum"], json!(0));
    assert_eq!(schema["maximum"], json!(100));
}

#[test]
fn integer_exclusive_min_max() {
    let schema = scheme::integer().exclusive_min(0).exclusive_max(100).build_schema();
    assert_eq!(schema["exclusiveMinimum"], json!(0));
    assert_eq!(schema["exclusiveMaximum"], json!(100));
}

#[test]
fn integer_multiple_of() {
    let schema = scheme::integer().multiple_of(5).build_schema();
    assert_eq!(schema["multipleOf"], json!(5));
}

#[test]
fn integer_positive_shorthand() {
    let schema = scheme::integer().positive().build_schema();
    assert_eq!(schema["exclusiveMinimum"], json!(0));
}

#[test]
fn integer_nonnegative_shorthand() {
    let schema = scheme::integer().nonnegative().build_schema();
    assert_eq!(schema["minimum"], json!(0));
}

#[test]
fn integer_negative_shorthand() {
    let schema = scheme::integer().negative().build_schema();
    assert_eq!(schema["exclusiveMaximum"], json!(0));
}

#[test]
fn integer_nonpositive_shorthand() {
    let schema = scheme::integer().nonpositive().build_schema();
    assert_eq!(schema["maximum"], json!(0));
}

#[test]
fn integer_enum_values() {
    let schema = scheme::integer()
        .enum_values(vec![1, 2, 3])
        .build_schema();
    assert_eq!(schema["enum"], json!([1, 2, 3]));
}

#[test]
fn integer_description_and_default() {
    let schema = scheme::integer()
        .description("Port number")
        .default(json!(8080))
        .build_schema();
    assert_eq!(schema["description"], "Port number");
    assert_eq!(schema["default"], 8080);
}

#[test]
fn integer_nullable() {
    let schema = scheme::integer().nullable().build_schema();
    assert_eq!(schema["type"], json!(["integer", "null"]));
}

#[test]
fn integer_optional() {
    let schema = scheme::integer().optional().build_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of[0]["type"], "integer");
    assert_eq!(any_of[1]["type"], "null");
}

#[test]
fn integer_port_number() {
    let schema = scheme::integer()
        .min(1)
        .max(65535)
        .description("Network port")
        .default(json!(8080))
        .build_schema();
    assert_eq!(schema["type"], "integer");
    assert_eq!(schema["minimum"], 1);
    assert_eq!(schema["maximum"], 65535);
    assert_eq!(schema["description"], "Network port");
    assert_eq!(schema["default"], 8080);
}

// ── Number Schema ──────────────────────────────────────────────

#[test]
fn number_default_schema() {
    let schema = scheme::number().build_schema();
    assert_eq!(schema, json!({"type": "number"}));
}

#[test]
fn number_min_max() {
    let schema = scheme::number().min(0.0).max(100.0).build_schema();
    assert_eq!(schema["minimum"], json!(0.0));
    assert_eq!(schema["maximum"], json!(100.0));
}

#[test]
fn number_exclusive_min_max() {
    let schema = scheme::number().exclusive_min(0.0).exclusive_max(1.0).build_schema();
    assert_eq!(schema["exclusiveMinimum"], json!(0.0));
    assert_eq!(schema["exclusiveMaximum"], json!(1.0));
}

#[test]
fn number_multiple_of() {
    let schema = scheme::number().multiple_of(0.5).build_schema();
    assert_eq!(schema["multipleOf"], json!(0.5));
}

#[test]
fn number_positive_shorthand() {
    let schema = scheme::number().positive().build_schema();
    assert_eq!(schema["exclusiveMinimum"], json!(0.0));
}

#[test]
fn number_nonnegative_shorthand() {
    let schema = scheme::number().nonnegative().build_schema();
    assert_eq!(schema["minimum"], json!(0.0));
}

#[test]
fn number_percentage() {
    let schema = scheme::number()
        .min(0.0)
        .max(100.0)
        .description("Percentage value")
        .build_schema();
    assert_eq!(schema["type"], "number");
    assert_eq!(schema["minimum"], 0.0);
    assert_eq!(schema["maximum"], 100.0);
    assert_eq!(schema["description"], "Percentage value");
}

#[test]
fn number_nullable() {
    let schema = scheme::number().nullable().build_schema();
    assert_eq!(schema["type"], json!(["number", "null"]));
}

#[test]
fn number_optional() {
    let schema = scheme::number().optional().build_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of[0]["type"], "number");
    assert_eq!(any_of[1]["type"], "null");
}

// ── Boolean Schema ──────────────────────────────────────────────

#[test]
fn boolean_default_schema() {
    let schema = scheme::boolean().build_schema();
    assert_eq!(schema, json!({"type": "boolean"}));
}

#[test]
fn boolean_description() {
    let schema = scheme::boolean().description("Is active flag").build_schema();
    assert_eq!(schema["description"], "Is active flag");
}

#[test]
fn boolean_default_value() {
    let schema = scheme::boolean().default(json!(true)).build_schema();
    assert_eq!(schema["default"], true);
}

#[test]
fn boolean_nullable() {
    let schema = scheme::boolean().nullable().build_schema();
    assert_eq!(schema["type"], json!(["boolean", "null"]));
}

#[test]
fn boolean_optional() {
    let schema = scheme::boolean().optional().build_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of[0]["type"], "boolean");
    assert_eq!(any_of[1]["type"], "null");
}

// ── Null Schema ────────────────────────────────────────────────

#[test]
fn null_default_schema() {
    let schema = scheme::null().build_schema();
    assert_eq!(schema, json!({"type": "null"}));
}

#[test]
fn null_description() {
    let schema = scheme::null().description("Explicitly null").build_schema();
    assert_eq!(schema["description"], "Explicitly null");
}

// ── build_schema is non-consuming ──────────────────────────────

#[test]
fn build_schema_can_be_called_multiple_times() {
    let builder = scheme::string().min_len(1).max_len(10);
    let schema1 = builder.build_schema();
    let schema2 = builder.build_schema();
    assert_eq!(schema1, schema2);
}

// ── From impls — builders convert to Value automatically ───────

#[test]
fn from_string_schema() {
    let value: serde_json::Value = scheme::string().min_len(1).into();
    assert_eq!(value["type"], "string");
    assert_eq!(value["minLength"], 1);
}

#[test]
fn from_integer_schema() {
    let value: serde_json::Value = scheme::integer().min(0).into();
    assert_eq!(value["type"], "integer");
    assert_eq!(value["minimum"], 0);
}

#[test]
fn from_number_schema() {
    let value: serde_json::Value = scheme::number().max(1.0).into();
    assert_eq!(value["type"], "number");
    assert_eq!(value["maximum"], 1.0);
}

#[test]
fn from_boolean_schema() {
    let value: serde_json::Value = scheme::boolean().into();
    assert_eq!(value["type"], "boolean");
}

#[test]
fn from_null_schema() {
    let value: serde_json::Value = scheme::null().into();
    assert_eq!(value["type"], "null");
}
