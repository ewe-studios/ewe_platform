//! Tests for foundation_arrow IPC encode/decode.

use arrow_array::{
    ArrayRef, BinaryArray, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema};
use foundation_arrow::{decode_ipc, decode_ipc_batches, encode_ipc, encode_ipc_batches};
use std::sync::Arc;

fn make_test_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("value", DataType::Float64, true),
        Field::new("timestamp", DataType::Int64, false),
        Field::new("data", DataType::Binary, true),
    ]));

    let id: ArrayRef = Arc::new(UInt64Array::from(vec![1, 2, 3]));
    let name: ArrayRef = Arc::new(StringArray::from(vec!["alice", "bob", "charlie"]));
    let value: ArrayRef = Arc::new(Float64Array::from(vec![
        Some(1.5),
        Some(2.7),
        None,
    ]));
    let timestamp: ArrayRef = Arc::new(Int64Array::from(vec![1000, 2000, 3000]));
    let data: ArrayRef = Arc::new(BinaryArray::from_opt_vec(vec![
        Some(b"hello"),
        None,
        Some(b"world"),
    ]));

    RecordBatch::try_new(schema, vec![id, name, value, timestamp, data]).unwrap()
}

#[test]
fn test_encode_decode_single_batch() {
    let batch = make_test_batch();
    let encoded = encode_ipc(&batch).expect("encode failed");

    // Encoded data should be non-empty
    assert!(!encoded.is_empty());

    let decoded = decode_ipc(&encoded).expect("decode failed");

    // Verify schema matches
    assert_eq!(batch.schema(), decoded.schema());

    // Verify row count
    assert_eq!(batch.num_rows(), decoded.num_rows());

    // Verify a column value
    let id_col = decoded
        .column(0)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    assert_eq!(id_col.value(0), 1);
    assert_eq!(id_col.value(1), 2);
    assert_eq!(id_col.value(2), 3);

    // Verify string column
    let name_col = decoded
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(name_col.value(0), "alice");
    assert_eq!(name_col.value(1), "bob");
}

#[test]
fn test_encode_decode_multiple_batches() {
    let batch1 = make_test_batch();
    let batch2 = make_test_batch(); // same schema, same data

    let encoded = encode_ipc_batches(&[batch1.clone(), batch2.clone()]).expect("encode failed");
    let decoded = decode_ipc_batches(&encoded).expect("decode failed");

    assert_eq!(decoded.len(), 2);

    // Each decoded batch should have the same schema
    assert_eq!(decoded[0].schema(), batch1.schema());
    assert_eq!(decoded[1].schema(), batch2.schema());

    // Combined rows should be 6
    let total_rows: usize = decoded.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 6);
}

#[test]
fn test_empty_batch_error() {
    let result = encode_ipc_batches(&[]);
    assert!(result.is_err());
}

#[test]
fn test_single_from_multi_batch_error() {
    let batch = make_test_batch();
    let encoded = encode_ipc_batches(&[batch, make_test_batch()]).expect("encode failed");
    let result = decode_ipc(&encoded);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("expected exactly one"));
}
