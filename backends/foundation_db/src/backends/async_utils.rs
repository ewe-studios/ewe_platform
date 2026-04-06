//! Shared async utilities for SQL backends (Turso, libsql).
//!
//! Provides two patterns for wrapping async futures using Valtron's
//! `from_future` + `execute` pattern:
//!
//! - **`schedule_future`** (preferred): Schedules work and returns a stream.
//!   Errors are preserved via `map_circuit` at the task level, yielding
//!   `Stream::Next(Err(e))` so callers can handle them.
//!
//! - **`exec_future`** (legacy): Blocks immediately at the leaf. Use only for
//!   one-shot initialization (DB connection, migrations), not for trait methods.

use foundation_core::valtron::{execute, from_future, Stream, StreamIteratorExt};

use crate::errors::StorageError;

/// WHY: Enables non-blocking, composable storage operations with error preservation.
///
/// WHAT: Schedules a future for execution via Valtron, returning a stream.
/// Errors are preserved in the stream as `Stream::Next(Err(e))` rather than
/// logging and swallowing them. Backend errors are converted to `StorageError`.
///
/// HOW: `from_future` → `execute` → `map_done(convert errors)` → `map_pending(erase)` → box.
///
/// # Errors
///
/// Returns a `StorageError` if Valtron scheduling fails.
pub fn schedule_future<T, E, F>(
    future: F,
) -> Result<
    impl foundation_core::valtron::StreamIterator<D = Result<T, StorageError>, P = ()> + Send + 'static,
    StorageError,
>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Into<StorageError> + Send + 'static,
{
    let task = from_future(future);

    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;

    // Convert backend errors to StorageError while preserving them in the stream
    Ok(stream
        .map_done(|result: Result<T, E>| result.map_err(Into::into))
        .map_pending(|_| ()))
}

/// WHY: One-shot blocking bridge for initialization and migrations.
///
/// WHAT: Wraps a future using Valtron's `from_future` + `execute` pattern,
/// blocking until the result is available. **Use sparingly** — prefer
/// `schedule_future` for trait methods to preserve composability.
///
/// HOW: Schedules the future, then eagerly drains the stream to extract
/// the single result.
///
/// # Errors
///
/// Returns a `StorageError` if:
/// - Valtron execution fails
/// - The future completes but returns an error
/// - No result is produced by the future execution
pub fn exec_future<T, E, F>(future: F) -> Result<T, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    F::Output: Send + 'static,
    T: Send + 'static,
    E: Into<StorageError> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron execution failed: {e}")))?;

    let mut result: Option<Result<T, StorageError>> = None;
    for item in stream {
        if let Stream::Next(v) = item {
            result = Some(v.map_err(Into::into));
            break;
        }
    }

    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => Err(StorageError::Generic(
            "No result from future execution".into(),
        )),
    }
}
