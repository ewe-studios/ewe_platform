//! Shared async utilities for SQL backends (Turso, libsql, D1 wasm, R2 wasm).
//!
//! Provides two patterns using valtron's unified executor:
//!
//! - **`schedule_future`**: Schedules a future via `from_future` → `execute` → `Stream`.
//!   Errors are preserved in the stream as `Stream::Next(Err(e))`.
//!
//! - **`exec_future`**: One-shot blocking bridge for initialization and migrations.
//!   Uses `execute` + collects first `Stream::Next` value.

// ============================================================================
// Native multi-threaded: uses StreamIteratorExt on DrivenStreamIterator
// ============================================================================

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use crate::core::errors::StorageError;
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use crate::core::storage_provider::StorageItemStream;
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use foundation_core::valtron::{from_future, execute, Stream, StreamIteratorExt};

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
pub fn schedule_future<T, E, F>(
    future: F,
) -> Result<StorageItemStream<'static, T>, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    F::Output: Send + 'static,
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

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
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
        None => Err(StorageError::Generic("No result from future execution".into())),
    }
}

// ============================================================================
// WASM32: execute returns Iterator<Item=Stream> — use plain Iterator::map
// ============================================================================

#[cfg(target_arch = "wasm32")]
use crate::core::errors::StorageError;
#[cfg(target_arch = "wasm32")]
use crate::core::storage_provider::StorageItemStream;
#[cfg(target_arch = "wasm32")]
use foundation_core::valtron::{from_future, execute, Stream};

#[cfg(target_arch = "wasm32")]
pub fn schedule_future<T, E, F>(
    future: F,
) -> Result<StorageItemStream<'static, T>, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
    F::Output: 'static,
    T: Send + 'static,
    E: Into<StorageError> + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
    Ok(Box::new(stream.map(|item| match item {
        Stream::Next(result) => Stream::Next(result.map_err(Into::into)),
        Stream::Pending(_) => Stream::Pending(()),
        Stream::Init => Stream::Init,
        Stream::Ignore => Stream::Ignore,
        Stream::Wait => Stream::Wait,
        Stream::Delayed(d) => Stream::Delayed(d),
        Stream::Spread(items) => Stream::Spread(items.into_iter().map(|item| match item {
            foundation_core::valtron::StreamSpread::Done(result) =>
                foundation_core::valtron::StreamSpread::Done(result.map_err(Into::into)),
            foundation_core::valtron::StreamSpread::Pending(_) =>
                foundation_core::valtron::StreamSpread::Pending(()),
        }).collect()),
    })))
}

#[cfg(target_arch = "wasm32")]
pub fn exec_future<T, E, F>(future: F) -> Result<T, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
    F::Output: 'static,
    T: Send + 'static,
    E: Into<StorageError> + 'static,
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
        None => Err(StorageError::Generic("No result from future execution".into())),
    }
}

// ============================================================================
// Native single-threaded (no multi, not wasm32) — same as wasm path
// ============================================================================

#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
use crate::core::errors::StorageError;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
use crate::core::storage_provider::StorageItemStream;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
use foundation_core::valtron::{from_future, execute, Stream};

#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
pub fn schedule_future<T, E, F>(
    future: F,
) -> Result<StorageItemStream<'static, T>, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
    F::Output: 'static,
    T: Send + 'static,
    E: Into<StorageError> + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
    Ok(Box::new(stream.map(|item| match item {
        Stream::Next(result) => Stream::Next(result.map_err(Into::into)),
        Stream::Pending(_) => Stream::Pending(()),
        Stream::Init => Stream::Init,
        Stream::Ignore => Stream::Ignore,
        Stream::Wait => Stream::Wait,
        Stream::Delayed(d) => Stream::Delayed(d),
        Stream::Spread(items) => Stream::Spread(items.into_iter().map(|item| match item {
            foundation_core::valtron::StreamSpread::Done(result) =>
                foundation_core::valtron::StreamSpread::Done(result.map_err(Into::into)),
            foundation_core::valtron::StreamSpread::Pending(_) =>
                foundation_core::valtron::StreamSpread::Pending(()),
        }).collect()),
    })))
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
pub fn exec_future<T, E, F>(future: F) -> Result<T, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
    F::Output: 'static,
    T: Send + 'static,
    E: Into<StorageError> + 'static,
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
        None => Err(StorageError::Generic("No result from future execution".into())),
    }
}
