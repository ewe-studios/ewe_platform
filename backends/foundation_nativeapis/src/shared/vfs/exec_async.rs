//! Blocking bridge for executing async VFS operations via valtron.

use foundation_core::valtron::{collect_one, execute, from_future};
use foundation_errstacks::ErrorTrace;

use super::error::{VfsError, VfsResult};

/// Executes an async future synchronously using the valtron runtime.
///
/// Wraps the given future into a valtron task, executes it, and collects a
/// single result value.
///
/// # Errors
///
/// Returns [`VfsError::Backend`] if valtron execution fails or produces no result.
pub fn exec_async<T: Send + 'static, F>(future: F) -> VfsResult<T>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
{
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
