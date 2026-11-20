//! Conversion between columnar batches and shared operations.
use alloc::vec::Vec;
use core::fmt;

use crate::{ColumnarBatch, ColumnarBatchT, UiOperationT};

use super::WriteError;

mod decode;
mod encode;

/// The reason a columnar batch could not be read or built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnarError {
    LengthMismatch {
        column: &'static str,
        expected: usize,
        actual: usize,
    },
    BadOffsets {
        row: usize,
    },
    UnknownOp {
        row: usize,
        code: u8,
    },
    UnsupportedOp {
        row: usize,
        code: u8,
    },
    InvalidUtf8 {
        row: usize,
    },
    InvalidValue {
        row: usize,
    },
    BlobTooLarge {
        row: usize,
    },
}

impl fmt::Display for ColumnarError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch {
                column,
                expected,
                actual,
            } => write!(
                output,
                "Column {column} has length {actual}, expected {expected}."
            ),
            Self::BadOffsets { row } => write!(output, "Row {row} has invalid blob offsets."),
            Self::UnknownOp { row, code } => {
                write!(output, "Row {row} has unknown operation number {code}.")
            }
            Self::UnsupportedOp { row, code } => {
                write!(
                    output,
                    "Row {row} uses operation {code}, which requires its normal form."
                )
            }
            Self::InvalidUtf8 { row } => write!(output, "Row {row} has invalid UTF-8 text."),
            Self::InvalidValue { row } => write!(output, "Row {row} has an invalid value."),
            Self::BlobTooLarge { row } => {
                write!(output, "The blob offset of row {row} is too large.")
            }
        }
    }
}

impl core::error::Error for ColumnarError {}

/// Builds canonical columns for the supplied shared operations.
///
/// # Errors
///
/// Returns an error for unsupported operations, invalid values, or an oversized blob.
pub fn columnar_batch(rows: &[UiOperationT]) -> Result<ColumnarBatchT, ColumnarError> {
    encode::batch(rows)
}

/// Reads the shared operations in a verified columnar table.
///
/// A zero-length text field becomes absent.
///
/// # Errors
///
/// Returns an error for invalid column lengths, offsets, operation numbers or values.
pub fn columnar_rows(batch: ColumnarBatch<'_>) -> Result<Vec<UiOperationT>, ColumnarError> {
    decode::rows(&batch.unpack())
}

pub(super) fn decode_batch(
    batch: &ColumnarBatchT,
    message_index: usize,
) -> Result<Vec<UiOperationT>, WriteError> {
    decode::rows(batch).map_err(|error| WriteError::Columnar {
        message_index,
        error,
    })
}

pub(super) fn encode_batch(
    rows: &[UiOperationT],
    message_index: usize,
) -> Result<ColumnarBatchT, WriteError> {
    columnar_batch(rows).map_err(|error| WriteError::Columnar {
        message_index,
        error,
    })
}
