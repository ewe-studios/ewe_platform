//! Arrow IPC encoding/decoding.
//!
//! Provides functions to serialize `RecordBatch`es to/from the Arrow IPC
//! file format, enabling efficient data transfer between processes.

use std::io::Cursor;

use arrow_array::RecordBatch;
use arrow_ipc::reader::FileReaderBuilder;
use arrow_ipc::writer::{FileWriter, IpcWriteOptions};
use arrow_schema::SchemaRef;

/// Result type for Arrow IPC operations.
pub type IpcResult<T> = Result<T, arrow_schema::ArrowError>;

/// Encode a `RecordBatch` to an Arrow IPC file format byte vector.
///
/// Uses the default IPC write options (8-byte alignment, little-endian).
pub fn encode_ipc(batch: &RecordBatch) -> IpcResult<Vec<u8>> {
    encode_ipc_with_options(batch, IpcWriteOptions::default())
}

/// Encode a `RecordBatch` to an Arrow IPC file format byte vector
/// with custom write options.
pub fn encode_ipc_with_options(
    batch: &RecordBatch,
    options: IpcWriteOptions,
) -> IpcResult<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut writer = FileWriter::try_new_with_options(&mut buffer, batch.schema_ref(), options)?;
        writer.write(batch)?;
        writer.finish()?;
    }
    Ok(buffer)
}

/// Encode multiple `RecordBatch`es to an Arrow IPC file format byte vector.
pub fn encode_ipc_batches(batches: &[RecordBatch]) -> IpcResult<Vec<u8>> {
    if batches.is_empty() {
        return Err(arrow_schema::ArrowError::IpcError(
            "no batches to encode".to_string(),
        ));
    }
    let schema = batches[0].schema_ref();
    let mut buffer = Vec::new();
    {
        let mut writer = FileWriter::try_new(&mut buffer, schema)?;
        for batch in batches {
            writer.write(batch)?;
        }
        writer.finish()?;
    }
    Ok(buffer)
}

/// Decode an Arrow IPC file format byte vector into a single `RecordBatch`.
///
/// Returns an error if the IPC file contains zero or multiple batches.
/// Use [`decode_ipc_batches`] for multi-batch files.
pub fn decode_ipc(data: &[u8]) -> IpcResult<RecordBatch> {
    let mut batches = decode_ipc_batches(data)?;
    match batches.len() {
        1 => Ok(batches.remove(0)),
        0 => Err(arrow_schema::ArrowError::IpcError(
            "IPC file contains no batches".to_string(),
        )),
        n => Err(arrow_schema::ArrowError::IpcError(format!(
            "IPC file contains {n} batches, expected exactly one"
        ))),
    }
}

/// Decode an Arrow IPC file format byte vector into all contained batches.
pub fn decode_ipc_batches(data: &[u8]) -> IpcResult<Vec<RecordBatch>> {
    let cursor = Cursor::new(data);
    let reader = FileReaderBuilder::new().build(cursor)?;
    reader.into_iter().collect::<Result<Vec<_>, _>>()
}

/// Decode an Arrow IPC file format byte vector and return the schema
/// without reading any batch data.
pub fn decode_ipc_schema(data: &[u8]) -> IpcResult<SchemaRef> {
    let cursor = Cursor::new(data);
    let reader = FileReaderBuilder::new().build(cursor)?;
    Ok(reader.schema())
}
