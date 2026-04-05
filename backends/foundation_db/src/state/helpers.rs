//! Stream consumption helpers for state store callers.
//!
//! WHY: Callers need convenient ways to extract values from
//! `StateStoreStream<T>` at sync boundaries without writing match boilerplate.
//!
//! WHAT: Three helpers — `collect_first` for single-value ops, `collect_all`
//! for multi-value ops, `drive_to_completion` for write ops.
//!
//! HOW: Drain the iterator, match `ThreadedValue::Value`, propagate errors.

use foundation_core::valtron::ThreadedValue;

use crate::errors::StorageError;
use super::traits::StateStoreStream;

/// Extract the first successful value from a state store stream.
///
/// Returns `Ok(None)` if the stream is empty.
///
/// # Errors
///
/// Returns the first error encountered in the stream.
pub fn collect_first<T>(mut stream: StateStoreStream<T>) -> Result<Option<T>, StorageError> {
    if let Some(item) = stream.next() {
        match item {
            ThreadedValue::Value(Ok(val)) => return Ok(Some(val)),
            ThreadedValue::Value(Err(e)) => return Err(e),
        }
    }
    Ok(None)
}

/// Collect all successful values from a state store stream.
///
/// # Errors
///
/// Returns the first error encountered in the stream.
pub fn collect_all<T>(stream: StateStoreStream<T>) -> Result<Vec<T>, StorageError> {
    let mut results = Vec::new();
    for item in stream {
        match item {
            ThreadedValue::Value(Ok(val)) => results.push(val),
            ThreadedValue::Value(Err(e)) => return Err(e),
        }
    }
    Ok(results)
}

/// Drive a write stream to completion, consuming all items.
///
/// # Errors
///
/// Returns the first error encountered in the stream.
pub fn drive_to_completion(stream: StateStoreStream<()>) -> Result<(), StorageError> {
    for item in stream {
        match item {
            ThreadedValue::Value(Ok(())) => {}
            ThreadedValue::Value(Err(e)) => return Err(e),
        }
    }
    Ok(())
}
