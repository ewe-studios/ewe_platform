//! Shared async utilities for SQL backends (Turso, libsql).
//!
//! Provides two patterns for wrapping async futures using Valtron's
//! `from_future` + `execute` pattern:
//!
//! - **`schedule_future`** (preferred): Schedules work and returns a boxed stream.
//!   Errors are preserved via `map_circuit` at the task level, yielding
//!   `Stream::Next(Err(e))` so callers can handle them.
//!
//! - **`exec_future`** (legacy): Blocks immediately at the leaf. Use only for
//!   one-shot initialization (DB connection, migrations), not for trait methods.

use crate::core::errors::StorageError;
use crate::core::storage_provider::StorageItemStream;
use foundation_core::valtron::Stream;

// ============================================================================
// Native path: Send-required via unified executor
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
use foundation_core::valtron::{execute, from_future, StreamIteratorExt};

/// WHY: Enables non-blocking, composable storage operations with error preservation.
///
/// WHAT: Schedules a future for execution via Valtron unified executor,
/// returning a boxed stream. Errors are preserved in the stream as
/// `Stream::Next(Err(e))`.
///
/// HOW: `from_future` → `execute` → `map_done(convert errors)` → `map_pending(erase)` → box.
///
/// # Errors
///
/// Returns a `StorageError` if Valtron scheduling fails.
#[cfg(not(target_arch = "wasm32"))]
pub fn schedule_future<T, E, F>(
    future: F,
) -> Result<StorageItemStream<'static, T>, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Into<StorageError> + Send + 'static,
{
    let task = from_future(future);

    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;

    Ok(Box::new(
        stream
            .map_done(|result: Result<T, E>| result.map_err(Into::into))
            .map_pending(|_| ()),
    ))
}

/// WHY: One-shot blocking bridge for initialization and migrations.
///
/// WHAT: Wraps a future using Valtron's `from_future` + `execute` pattern,
/// blocking until the result is available.
///
/// # Errors
///
/// Returns a `StorageError` if scheduling fails or the future returns an error.
#[cfg(not(target_arch = "wasm32"))]
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

// ============================================================================
// WASM32 path: non-Send via drive_non_send_iterator
// ============================================================================

#[cfg(target_arch = "wasm32")]
use foundation_core::valtron::{
    drive_non_send_iterator, from_future_non_send, FuturePollState, NoAction, StreamIteratorExt,
    TaskStatus,
};

/// WHY: wasm32 futures (JsFuture etc.) are !Send. On wasm32 single-threaded target
/// Send is structurally safe but types don't implement it.
///
/// WHAT: `from_future_non_send` → `drive_non_send_iterator` → map errors → box.
/// `drive_non_send_iterator` internally calls `run_until_next_state()` before each
/// `next()`, driving the valtron executor so the scheduled task makes progress.
///
/// # Errors
///
/// Returns a `StorageError` if Valtron scheduling fails.
#[cfg(target_arch = "wasm32")]
pub fn schedule_future<T, E, F>(
    future: F,
) -> Result<StorageItemStream<'static, T>, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Into<StorageError> + 'static,
{
    let task = from_future_non_send(future);
    let mut driven = drive_non_send_iterator(task);

    Ok(Box::new(
        std::iter::from_fn(move || {
            driven.next().and_then(|status| match status {
                TaskStatus::Ready(result) => Some(Stream::Next(result.map_err(Into::into))),
                TaskStatus::Pending(_) => Some(Stream::Pending(())),
                TaskStatus::Init => Some(Stream::Init),
                TaskStatus::Delayed(d) => Some(Stream::Delayed(d)),
                TaskStatus::Ignore | TaskStatus::Wait => None,
                TaskStatus::Spawn(_) => None,
            })
        }),
    ))
}

/// WHY: One-shot blocking bridge for initialization and migrations on wasm32.
///
/// WHAT: `from_future_non_send` → `drive_non_send_iterator` → extract first Ready result.
///
/// # Errors
///
/// Returns a `StorageError` if the future returns an error.
#[cfg(target_arch = "wasm32")]
pub fn exec_future<T, E, F>(future: F) -> Result<T, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
    F::Output: 'static,
    T: 'static,
    E: Into<StorageError> + 'static,
{
    let task = from_future_non_send(future);
    let mut driven = drive_non_send_iterator(task);

    for status in driven.by_ref() {
        if let TaskStatus::Ready(v) = status {
            return v.map_err(Into::into);
        }
    }

    Err(StorageError::Generic(
        "No result from future execution".into(),
    ))
}
