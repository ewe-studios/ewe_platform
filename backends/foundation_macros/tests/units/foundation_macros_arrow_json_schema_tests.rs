// Derive-macro fixtures: fields exist to drive ArrowSchema/ArrowJsonSchema
// generation (their NAMES are asserted in the emitted schema), so they are
// never read at runtime and must not be renamed to dodge the prefix lint.
#![allow(dead_code, clippy::struct_field_names)]

use foundation_arrow::ArrowJsonSchema;
use foundation_macros::{ArrowJsonSchema, ArrowSchema};

#[derive(ArrowSchema, ArrowJsonSchema)]
struct MetadataRecord {
    name: String,
    size: u64,
    permissions: u32,
    checksum: Option<Vec<u8>>,
}

#[test]
fn test_derive_arrow_json_schema() {
    let schema = MetadataRecord::arrow_json_schema();

    let fields = schema["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 4);

    assert_eq!(fields[0]["name"], "name");
    assert_eq!(fields[0]["data_type"], "Utf8");
    assert_eq!(fields[0]["nullable"], false);

    assert_eq!(fields[1]["name"], "size");
    assert_eq!(fields[1]["data_type"], "UInt64");
    assert_eq!(fields[1]["nullable"], false);

    assert_eq!(fields[2]["name"], "permissions");
    assert_eq!(fields[2]["data_type"], "UInt32");
    assert_eq!(fields[2]["nullable"], false);

    assert_eq!(fields[3]["name"], "checksum");
    assert_eq!(fields[3]["data_type"], "Binary");
    assert_eq!(fields[3]["nullable"], true);
}

#[derive(ArrowSchema, ArrowJsonSchema)]
struct NumericRecord {
    val_f32: f32,
    val_f64: f64,
    val_bool: bool,
}

#[test]
fn test_derive_arrow_json_schema_float_bool() {
    let schema = NumericRecord::arrow_json_schema();
    let fields = schema["fields"].as_array().unwrap();

    assert_eq!(fields[0]["data_type"], "Float32");
    assert_eq!(fields[1]["data_type"], "Float64");
    assert_eq!(fields[2]["data_type"], "Boolean");
}
