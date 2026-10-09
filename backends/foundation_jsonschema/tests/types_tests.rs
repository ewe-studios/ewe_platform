use foundation_jsonschema::{JsonType, JsonTypeSet};
use serde_json::Value;

#[test]
fn test_json_type_of_null() {
    assert_eq!(JsonType::of(&Value::Null), JsonType::Null);
}

#[test]
fn test_json_type_of_boolean() {
    assert_eq!(JsonType::of(&Value::Bool(true)), JsonType::Boolean);
    assert_eq!(JsonType::of(&Value::Bool(false)), JsonType::Boolean);
}

#[test]
fn test_json_type_of_integer() {
    assert_eq!(JsonType::of(&Value::Number(42.into())), JsonType::Integer);
}

#[test]
fn test_json_type_of_number() {
    let n: Value = serde_json::Number::from_f64(std::f64::consts::PI)
        .unwrap()
        .into();
    assert_eq!(JsonType::of(&n), JsonType::Number);
}

#[test]
fn test_json_type_of_string() {
    assert_eq!(
        JsonType::of(&Value::String("hello".into())),
        JsonType::String
    );
}

#[test]
fn test_json_type_of_array() {
    assert_eq!(JsonType::of(&Value::Array(vec![])), JsonType::Array);
}

#[test]
fn test_json_type_of_object() {
    assert_eq!(
        JsonType::of(&Value::Object(Default::default())),
        JsonType::Object
    );
}

#[test]
fn test_json_type_set_insert_and_contains() {
    let mut set = JsonTypeSet::new();
    assert!(set.is_empty());
    set.insert(JsonType::String);
    set.insert(JsonType::Integer);
    assert_eq!(set.len(), 2);
    assert!(set.contains(JsonType::String));
    assert!(set.contains(JsonType::Integer));
    assert!(!set.contains(JsonType::Number));
}

#[test]
fn test_json_type_set_with() {
    let set = JsonTypeSet::with(JsonType::Boolean);
    assert_eq!(set.len(), 1);
    assert!(set.contains(JsonType::Boolean));
}

#[test]
fn test_json_type_set_iter() {
    let mut set = JsonTypeSet::new();
    set.insert(JsonType::Null);
    set.insert(JsonType::String);
    let types: Vec<JsonType> = set.iter().collect();
    assert_eq!(types, vec![JsonType::Null, JsonType::String]);
}

#[test]
fn test_json_type_set_from_schema_value_string() {
    let set = JsonTypeSet::from_schema_value(&Value::String("string".into())).unwrap();
    assert!(set.contains(JsonType::String));
}

#[test]
fn test_json_type_set_from_schema_value_array() {
    let value = Value::Array(vec![
        Value::String("string".into()),
        Value::String("number".into()),
    ]);
    let set = JsonTypeSet::from_schema_value(&value).unwrap();
    assert!(set.contains(JsonType::String));
    assert!(set.contains(JsonType::Number));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_json_type_set_from_schema_value_invalid() {
    assert!(JsonTypeSet::from_schema_value(&Value::Number(42.into())).is_none());
}

#[test]
fn test_json_type_display() {
    assert_eq!(format!("{}", JsonType::String), "string");
    assert_eq!(format!("{}", JsonType::Number), "number");
}

#[test]
fn test_json_type_set_display() {
    let mut set = JsonTypeSet::new();
    set.insert(JsonType::Null);
    set.insert(JsonType::String);
    assert_eq!(format!("{set}"), "(null, string)");
}
