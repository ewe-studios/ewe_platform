// Derive-macro fixtures: fields exist to drive ArrowSchema generation and
// are asserted via the emitted schema, never read directly at runtime.
#![allow(dead_code)]

use foundation_arrow::arrow_schema::DataType;
use foundation_arrow::ArrowSchema;
use foundation_macros::ArrowSchema;

#[derive(ArrowSchema)]
struct BasicRecord {
    name: String,
    count: u64,
    score: f64,
    active: bool,
}

#[test]
fn test_derive_arrow_schema_basic() {
    let fields = BasicRecord::fields();
    assert_eq!(fields.len(), 4);

    assert_eq!(fields[0].name(), "name");
    assert_eq!(fields[0].data_type(), &DataType::Utf8);
    assert!(!fields[0].is_nullable());

    assert_eq!(fields[1].name(), "count");
    assert_eq!(fields[1].data_type(), &DataType::UInt64);
    assert!(!fields[1].is_nullable());

    assert_eq!(fields[2].name(), "score");
    assert_eq!(fields[2].data_type(), &DataType::Float64);
    assert!(!fields[2].is_nullable());

    assert_eq!(fields[3].name(), "active");
    assert_eq!(fields[3].data_type(), &DataType::Boolean);
    assert!(!fields[3].is_nullable());
}

#[derive(ArrowSchema)]
struct WithOptionals {
    required_name: String,
    optional_size: Option<u64>,
    optional_data: Option<Vec<u8>>,
}

#[test]
fn test_derive_arrow_schema_option_nullable() {
    let fields = WithOptionals::fields();
    assert_eq!(fields.len(), 3);

    assert_eq!(fields[0].name(), "required_name");
    assert!(!fields[0].is_nullable());

    assert_eq!(fields[1].name(), "optional_size");
    assert_eq!(fields[1].data_type(), &DataType::UInt64);
    assert!(fields[1].is_nullable());

    assert_eq!(fields[2].name(), "optional_data");
    assert_eq!(fields[2].data_type(), &DataType::Binary);
    assert!(fields[2].is_nullable());
}

#[derive(ArrowSchema)]
struct AllIntegerTypes {
    a: u8,
    b: u16,
    c: u32,
    d: u64,
    e: i8,
    f: i16,
    g: i32,
    h: i64,
}

#[test]
fn test_derive_arrow_schema_all_integers() {
    let fields = AllIntegerTypes::fields();
    assert_eq!(fields[0].data_type(), &DataType::UInt8);
    assert_eq!(fields[1].data_type(), &DataType::UInt16);
    assert_eq!(fields[2].data_type(), &DataType::UInt32);
    assert_eq!(fields[3].data_type(), &DataType::UInt64);
    assert_eq!(fields[4].data_type(), &DataType::Int8);
    assert_eq!(fields[5].data_type(), &DataType::Int16);
    assert_eq!(fields[6].data_type(), &DataType::Int32);
    assert_eq!(fields[7].data_type(), &DataType::Int64);
}

#[derive(ArrowSchema)]
struct BinaryData {
    content: Vec<u8>,
}

#[test]
fn test_derive_arrow_schema_vec_u8_is_binary() {
    let fields = BinaryData::fields();
    assert_eq!(fields[0].data_type(), &DataType::Binary);
}

#[test]
fn test_derive_arrow_schema_produces_valid_schema() {
    let schema = BasicRecord::schema();
    assert_eq!(schema.fields().len(), 4);
    assert_eq!(schema.field(0).name(), "name");
}
