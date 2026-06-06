use serde_json::{json, Value};

use super::field_extractor::StructField;

#[must_use]
pub fn build_json_schema(struct_name: &str, fields: &[StructField]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in fields {
        properties.insert(field.name.clone(), rust_type_to_json_schema(&field.rust_type));
        if !field.nullable {
            required.push(Value::String(field.name.clone()));
        }
    }

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": struct_name,
        "type": "object",
        "properties": properties,
        "required": required
    })
}

fn rust_type_to_json_schema(ty: &str) -> Value {
    match ty {
        "u8" => json!({"type": "integer", "format": "uint8"}),
        "u16" => json!({"type": "integer", "format": "uint16"}),
        "u32" => json!({"type": "integer", "format": "uint32"}),
        "u64" => json!({"type": "integer", "format": "uint64"}),
        "i8" => json!({"type": "integer", "format": "int8"}),
        "i16" => json!({"type": "integer", "format": "int16"}),
        "i32" => json!({"type": "integer", "format": "int32"}),
        "i64" => json!({"type": "integer", "format": "int64"}),
        "f32" => json!({"type": "number", "format": "float32"}),
        "f64" => json!({"type": "number", "format": "float64"}),
        "bool" => json!({"type": "boolean"}),
        "String" => json!({"type": "string"}),
        "Vec<u8>" => json!({"type": "string", "format": "binary"}),
        "SystemTime" => json!({"type": "string", "format": "date-time"}),
        _ => json!({"type": "unknown"}),
    }
}
