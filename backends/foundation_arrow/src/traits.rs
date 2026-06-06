//! Core traits for Arrow serialization.
//!
//! Three traits form the foundation:
//! - [`ArrowSchema`] — generate an Arrow `Schema` for a type
//! - [`ToArrow`] — convert to an Arrow `RecordBatch`
//! - [`FromArrow`] — convert from an Arrow `RecordBatch`

use crate::ipc::IpcResult;
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_schema::{Field, Schema, SchemaRef};

/// Generate an Arrow `Schema` for a Rust type.
///
/// Implemented automatically via `#[derive(ArrowSchema)]` for structs.
pub trait ArrowSchema {
    /// Returns the Arrow schema for this type (single row).
    fn schema() -> Schema;

    /// Returns a `SchemaRef` for this type.
    fn schema_ref() -> SchemaRef {
        std::sync::Arc::new(Self::schema())
    }

    /// Returns the Arrow fields for this type.
    fn fields() -> Vec<Field>;
}

/// Convert a Rust type (or collection) to an Arrow `RecordBatch`.
///
/// Implemented automatically via `#[derive(ToArrow)]` for structs.
/// For collections, use `to_arrow_batch()` to produce a columnar batch.
pub trait ToArrow: ArrowSchema {
    /// Convert a single value to a `RecordBatch` with one row.
    fn to_arrow(&self) -> IpcResult<RecordBatch>;

    /// Convert a slice of values to a columnar `RecordBatch`.
    ///
    /// This is the efficient path for bulk data — each field becomes
    /// a single `ArrayRef` with `values.len()` elements.
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch>
    where
        Self: Sized;
}

/// Convert an Arrow `RecordBatch` back to a Rust type (or collection).
///
/// Implemented automatically via `#[derive(FromArrow)]` for structs.
pub trait FromArrow: ArrowSchema + Sized {
    /// Convert a single-row `RecordBatch` to this type.
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self>
    where
        Self: Sized;

    /// Convert a columnar `RecordBatch` to a `Vec` of this type.
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>>
    where
        Self: Sized;
}

/// Helper: extract a single value from an array at a given index.
pub trait ArrowValue<T> {
    fn arrow_value(&self, idx: usize) -> Option<T>;
}

impl ArrowValue<u8> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<u8> {
        use arrow_array::UInt8Array;
        self.as_any()
            .downcast_ref::<UInt8Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<u16> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<u16> {
        use arrow_array::UInt16Array;
        self.as_any()
            .downcast_ref::<UInt16Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<u32> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<u32> {
        use arrow_array::UInt32Array;
        self.as_any()
            .downcast_ref::<UInt32Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<u64> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<u64> {
        use arrow_array::UInt64Array;
        self.as_any()
            .downcast_ref::<UInt64Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<i8> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<i8> {
        use arrow_array::Int8Array;
        self.as_any()
            .downcast_ref::<Int8Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<i16> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<i16> {
        use arrow_array::Int16Array;
        self.as_any()
            .downcast_ref::<Int16Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<i32> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<i32> {
        use arrow_array::Int32Array;
        self.as_any()
            .downcast_ref::<Int32Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<i64> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<i64> {
        use arrow_array::Int64Array;
        self.as_any()
            .downcast_ref::<Int64Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<f32> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<f32> {
        use arrow_array::Float32Array;
        self.as_any()
            .downcast_ref::<Float32Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<f64> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<f64> {
        use arrow_array::Float64Array;
        self.as_any()
            .downcast_ref::<Float64Array>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<bool> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<bool> {
        use arrow_array::BooleanArray;
        self.as_any()
            .downcast_ref::<BooleanArray>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx)) })
    }
}

impl ArrowValue<String> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<String> {
        use arrow_array::StringArray;
        self.as_any()
            .downcast_ref::<StringArray>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx).to_string()) })
    }
}

impl ArrowValue<Vec<u8>> for ArrayRef {
    fn arrow_value(&self, idx: usize) -> Option<Vec<u8>> {
        use arrow_array::BinaryArray;
        self.as_any()
            .downcast_ref::<BinaryArray>()
            .and_then(|arr| if arr.is_null(idx) { None } else { Some(arr.value(idx).to_vec()) })
    }
}
