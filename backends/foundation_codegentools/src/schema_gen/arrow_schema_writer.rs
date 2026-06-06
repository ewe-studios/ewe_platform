use serde_json::{json, Value};

use super::field_extractor::StructField;

#[must_use]
pub fn build_arrow_schema(struct_name: &str, fields: &[StructField]) -> Value {
    let field_values: Vec<Value> = fields
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "data_type": rust_type_to_arrow_type(&f.rust_type),
                "nullable": f.nullable
            })
        })
        .collect();

    json!({
        "struct_name": struct_name,
        "fields": field_values
    })
}

fn rust_type_to_arrow_type(ty: &str) -> &'static str {
    match ty {
        "u8" => "UInt8",
        "u16" => "UInt16",
        "u32" => "UInt32",
        "u64" => "UInt64",
        "i8" => "Int8",
        "i16" => "Int16",
        "i32" => "Int32",
        "i64" => "Int64",
        "f32" => "Float32",
        "f64" => "Float64",
        "bool" => "Boolean",
        "String" => "Utf8",
        "Vec<u8>" => "Binary",
        "SystemTime" => "Timestamp(Millisecond, UTC)",
        _ => "Unknown",
    }
}
