//! Blocking bridge for executing async VFS operations via valtron.
//!
//! This is the sync boundary of the VFS: only the sync adapters (`SyncFs`
//! and its file and directory wrappers) call it.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use foundation_core::valtron::{collect_one, execute, from_future};
use foundation_errstacks::ErrorTrace;

use super::error::{VfsError, VfsResult};

/// Executes an async future synchronously.
///
/// The future is polled once on the calling thread first. Most VFS futures
/// (memory stores, the native file system, Fjall) are ready at once, so they
/// need no valtron pool and never use a worker. Only a future that is really
/// `Pending` goes to valtron, which drives it to completion.
///
/// # Errors
///
/// Returns [`VfsError::Backend`] if valtron execution fails or produces no result.
pub fn exec_async<T: Send + 'static, F>(future: F) -> VfsResult<T>
where
    F: Future<Output = VfsResult<T>> + Send + 'static,
{
    let mut future = Box::pin(future);
    let mut cx = Context::from_waker(Waker::noop());
    if let Poll::Ready(result) = future.as_mut().poll(&mut cx) {
        return result;
    }

    let task = from_future(future);
    let stream = execute(task, None).map_err(|e| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("valtron execution failed: {e}"),
        })
    })?;
    let result: Option<Result<T, ErrorTrace<VfsError>>> = collect_one(stream);
    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => Err(ErrorTrace::new(VfsError::Backend {
            message: "no result from async execution".into(),
        })),
    }
}

/// Polls a future once on the calling thread, with a no-op waker.
///
/// Returns `Some` when the future is ready at once. Used by sync adapters
/// that must not start a valtron task (for example a `Drop` impl).
pub fn poll_once<T, F: Future<Output = T>>(future: F) -> Option<T> {
    let future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.poll(&mut cx) {
        Poll::Ready(v) => Some(v),
        Poll::Pending => None,
    }
}
