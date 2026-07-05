//! Platform-agnostic HTTP exchange types for `TaskIterator`-based request tasks.
//!
//! WHY: The transport pump in connectrpc needs a single `TaskIterator` shape for HTTP
//! exchanges — native wraps `SendRequestTask<R>` (DNS + TCP + TLS + HTTP), WASM wraps
//! `fetch()` (browser Fetch API). Both produce the same `Ready`/`Pending` types so the
//! pump code is platform-agnostic.
//!
//! WHAT: [`HttpExchange`] (the `Ready` type), [`HttpExchangePending`] (the `Pending`
//! type). Each platform provides its own `HttpExchangeTask` in its own directory,
//! exported via `new_http_exchange_task(request, …)`.

use crate::simple_http::shared::{SimpleHeaders, Status};
use bytes::Bytes;
use foundation_core::extensions::result_ext::BoxedError;

/// What a platform HTTP request task yields on each poll.
///
/// Ordering guarantee: if the request succeeds, `Head` is yielded first (exactly
/// once), followed by zero or more `BodyChunk`s, then `None` (exhausted). If the
/// request fails, `Failed` is yielded (exactly once), then `None`.
pub enum HttpExchange {
    /// Response head. Exactly once per successful request.
    Head {
        status: Status,
        headers: SimpleHeaders,
    },
    /// One chunk of response body bytes. Zero or more.
    BodyChunk(Bytes),
    /// The request or response failed. Exactly once; no `Head` was produced.
    Failed(BoxedError),
}

/// Marker for the pending state — the task is waiting for I/O (DNS, TCP, TLS, or
/// the response stream).
#[derive(Debug, Clone, Copy)]
pub enum HttpExchangePending {
    Waiting,
}
