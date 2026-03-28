//! Shared async utilities for SQL backends (Turso, libsql).
//!
//! Provides two patterns for wrapping async futures using Valtron's
//! `from_future` + `execute` pattern:
//!
//! - **`schedule_future`** (preferred): Schedules work and returns a stream.
//!   The future's `Result<T, E>` is unwrapped (errors logged), and the
//!   `FuturePollState` is erased to `()`. Callers collect at boundaries
//!   using `collect_one`, `collect_result`, etc.
//!
//! - **`exec_future`** (legacy): Blocks immediately at the leaf. Use only for
//!   one-shot initialization (DB connection, migrations), not for trait methods.

use foundation_core::valtron::{execute, from_future, StreamIteratorExt};

use crate::errors::StorageError;

/// WHY: Enables non-blocking, composable storage operations.
///
/// WHAT: Schedules a future for execution via Valtron, returning a stream.
/// Uses `map_done` to unwrap `Result<T, E>` and `map_pending` to erase
/// `FuturePollState` → `()`, yielding a clean `StorageItemStream<T>`.
///
/// HOW: `from_future` → `execute` → `map_done(unwrap result)` → `map_pending(erase)` → box.
/// Errors from the future are logged via tracing and mapped to a sentinel value
/// using the caller-provided `on_err` function (commonly returns a type's default
/// or `None`). For methods where there's no meaningful fallback value, the error
/// path yields the sentinel which the caller can check.
///
/// # Errors
///
/// Returns a `StorageError` if Valtron scheduling fails.
pub fn schedule_future<T, E, F, H>(
    future: F,
    on_err: H,
) -> Result<impl foundation_core::valtron::StreamIterator<D = T, P = ()> + Send + 'static, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
    H: Fn(E) -> T + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;

    Ok(stream
        .map_done(move |result| match result {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Async execution error: {e}");
                on_err(e)
            }
        })
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
    E: std::fmt::Display + Send + 'static,
{
    use foundation_core::valtron::Stream;

    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron execution failed: {e}")))?;

    let mut result: Option<Result<T, E>> = None;
    for item in stream.into_iter() {
        if let Stream::Next(v) = item {
            result = Some(v);
            break;
        }
    }

    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(StorageError::Backend(format!("SQL error: {e}"))),
        None => Err(StorageError::Generic("No result from future execution".into())),
    }
}
