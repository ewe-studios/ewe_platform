//! Shared streamed body-read helpers for the auth HTTP clients.

use foundation_netio::shared::client::body_reader::{
    collect_bytes_async, collect_bytes_bounded, AsyncSendSafeBody,
};

/// Default cap on an error body read for its message (64 KiB).
///
/// It matches `foundation_deployment`'s `BodyLimits::max_error_body` and the
/// order of nginx's default in-memory proxy buffer size.
pub const DEFAULT_MAX_ERROR_BODY: usize = 64 * 1024;

/// Read a whole success body as lossy UTF-8 text.
///
/// JSON is valid UTF-8 in practice, so the lossy decode only masks a body that
/// is already malformed.
///
/// # Errors
///
/// Returns a boxed error message when the stream fails mid-read.
pub async fn read_body_text(body: AsyncSendSafeBody) -> Result<String, String> {
    let bytes = collect_bytes_async(body).await.map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Read an error body, at most `limit` bytes, as lossy UTF-8 text.
///
/// Bytes past `limit` are never read. A body that fails part-way yields the
/// text that arrived before the error.
pub async fn read_error_text(body: AsyncSendSafeBody, limit: usize) -> String {
    let bytes = collect_bytes_bounded(body, limit).await;
    String::from_utf8_lossy(&bytes).into_owned()
}
