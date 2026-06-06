use foundation_jsonschema::JsonSchema;
use foundation_macros::JsonSchema;

#[derive(JsonSchema)]
struct UserProfile {
    name: String,
    age: u32,
    bio: Option<String>,
}

#[test]
fn test_derive_json_schema_basic() {
    let schema = UserProfile::json_schema();

    assert_eq!(schema["type"], "object");

    let props = &schema["properties"];
    assert_eq!(props["name"]["type"], "string");
    assert_eq!(props["age"]["type"], "integer");
    assert_eq!(props["age"]["format"], "uint32");
    assert_eq!(props["bio"]["type"], "string");
}

#[test]
fn test_derive_json_schema_required() {
    let schema = UserProfile::json_schema();
    let required = schema["required"].as_array().unwrap();

    let required_names: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(required_names.contains(&"name"));
    assert!(required_names.contains(&"age"));
    assert!(!required_names.contains(&"bio"));
}

#[derive(JsonSchema)]
struct AllTypes {
    a: u8,
    b: u16,
    c: u32,
    d: u64,
    e: i8,
    f: i16,
    g: i32,
    h: i64,
    i: f32,
    j: f64,
    k: bool,
    l: String,
    m: Vec<u8>,
}

#[test]
fn test_derive_json_schema_all_types() {
    let schema = AllTypes::json_schema();
    let props = &schema["properties"];

    assert_eq!(props["a"]["format"], "uint8");
    assert_eq!(props["b"]["format"], "uint16");
    assert_eq!(props["c"]["format"], "uint32");
    assert_eq!(props["d"]["format"], "uint64");
    assert_eq!(props["e"]["format"], "int8");
    assert_eq!(props["f"]["format"], "int16");
    assert_eq!(props["g"]["format"], "int32");
    assert_eq!(props["h"]["format"], "int64");
    assert_eq!(props["i"]["format"], "float32");
    assert_eq!(props["j"]["format"], "float64");
    assert_eq!(props["k"]["type"], "boolean");
    assert_eq!(props["l"]["type"], "string");
    assert_eq!(props["m"]["format"], "binary");
}

#[derive(JsonSchema)]
struct AllOptional {
    x: Option<u64>,
    y: Option<String>,
}

#[test]
fn test_derive_json_schema_all_optional_empty_required() {
    let schema = AllOptional::json_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.is_empty());
}
