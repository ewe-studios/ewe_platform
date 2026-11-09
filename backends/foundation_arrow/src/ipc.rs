//! Arrow IPC encoding/decoding.
//!
//! Provides functions to serialize `RecordBatch`es to/from the Arrow IPC
//! file format, enabling efficient data transfer between processes.

use std::io::Cursor;

use arrow_array::RecordBatch;
use arrow_ipc::reader::FileReaderBuilder;
use arrow_ipc::writer::{FileWriter, IpcWriteOptions};
use arrow_schema::SchemaRef;
use derive_more::{Display, Error, From};
use foundation_errstacks::ErrorTrace;

/// Errors from Arrow IPC and row conversion operations.
#[derive(Debug, Display, Error, From)]
pub enum ArrowIpcError {
    /// The Arrow implementation rejected the schema, batch or IPC stream.
    #[display("arrow error: {_0}")]
    Arrow(arrow_schema::ArrowError),
}

impl From<ArrowIpcError> for ErrorTrace<ArrowIpcError> {
    fn from(err: ArrowIpcError) -> Self {
        ErrorTrace::new(err)
    }
}

/// Wrap an [`arrow_schema::ArrowError`] as an errstacks trace.
#[must_use]
pub fn arrow_err(err: arrow_schema::ArrowError) -> ErrorTrace<ArrowIpcError> {
    ArrowIpcError::from(err).into()
}

/// Result type for Arrow IPC operations.
pub type IpcResult<T> = Result<T, ErrorTrace<ArrowIpcError>>;

/// Encode a `RecordBatch` to an Arrow IPC file format byte vector.
///
/// Uses the default IPC write options (8-byte alignment, little-endian).
///
/// # Errors
/// Returns an error when the IPC writer rejects the batch.
pub fn encode_ipc(batch: &RecordBatch) -> IpcResult<Vec<u8>> {
    encode_ipc_with_options(batch, IpcWriteOptions::default())
}

/// Encode a `RecordBatch` to an Arrow IPC file format byte vector
/// with custom write options.
///
/// # Errors
/// Returns an error when the IPC writer rejects the batch or the options.
pub fn encode_ipc_with_options(
    batch: &RecordBatch,
    options: IpcWriteOptions,
) -> IpcResult<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut writer = FileWriter::try_new_with_options(&mut buffer, batch.schema_ref(), options)
            .map_err(ArrowIpcError::from)?;
        writer.write(batch).map_err(ArrowIpcError::from)?;
        writer.finish().map_err(ArrowIpcError::from)?;
    }
    Ok(buffer)
}

/// Encode multiple `RecordBatch`es to an Arrow IPC file format byte vector.
///
/// # Errors
/// Returns an error when `batches` is empty or the IPC writer rejects a batch.
pub fn encode_ipc_batches(batches: &[RecordBatch]) -> IpcResult<Vec<u8>> {
    if batches.is_empty() {
        return Err(ArrowIpcError::from(arrow_schema::ArrowError::IpcError(
            "no batches to encode".to_string(),
        ))
        .into());
    }
    let schema = batches[0].schema_ref();
    let mut buffer = Vec::new();
    {
        let mut writer = FileWriter::try_new(&mut buffer, schema).map_err(ArrowIpcError::from)?;
        for batch in batches {
            writer.write(batch).map_err(ArrowIpcError::from)?;
        }
        writer.finish().map_err(ArrowIpcError::from)?;
    }
    Ok(buffer)
}

/// Decode an Arrow IPC file format byte vector into a single `RecordBatch`.
///
/// Returns an error if the IPC file contains zero or multiple batches.
/// Use [`decode_ipc_batches`] for multi-batch files.
///
/// # Errors
/// Returns an error when the bytes are not a valid IPC file, or the file
/// contains zero or multiple batches.
pub fn decode_ipc(data: &[u8]) -> IpcResult<RecordBatch> {
    let mut batches = decode_ipc_batches(data)?;
    match batches.len() {
        1 => Ok(batches.remove(0)),
        0 => Err(ArrowIpcError::from(arrow_schema::ArrowError::IpcError(
            "IPC file contains no batches".to_string(),
        ))
        .into()),
        n => Err(
            ArrowIpcError::from(arrow_schema::ArrowError::IpcError(format!(
                "IPC file contains {n} batches, expected exactly one"
            )))
            .into(),
        ),
    }
}

/// Decode an Arrow IPC file format byte vector into all contained batches.
///
/// # Errors
/// Returns an error when the bytes are not a valid IPC file or a batch read fails.
pub fn decode_ipc_batches(data: &[u8]) -> IpcResult<Vec<RecordBatch>> {
    let cursor = Cursor::new(data);
    let reader = FileReaderBuilder::new()
        .build(cursor)
        .map_err(ArrowIpcError::from)?;
    reader
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(arrow_err)
}

/// Decode an Arrow IPC file format byte vector and return the schema
/// without reading any batch data.
///
/// # Errors
/// Returns an error when the bytes are not a valid IPC file.
pub fn decode_ipc_schema(data: &[u8]) -> IpcResult<SchemaRef> {
    let cursor = Cursor::new(data);
    let reader = FileReaderBuilder::new()
        .build(cursor)
        .map_err(ArrowIpcError::from)?;
    Ok(reader.schema())
}
