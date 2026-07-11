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

use std::sync::Arc;

use crate::shared::http::{SimpleHeaders, Status};
use bytes::Bytes;

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
    ///
    /// WHY `Arc` (F45 Resolution 7): a pre-head failure is fanned out to **both**
    /// split branches (head observer as `Err`, body continuation as `Err`), so the
    /// error payload must clone losslessly rather than being consumed once.
    Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

/// Marker for the pending state — the task is waiting for I/O (DNS, TCP, TLS, or
/// the response stream).
#[derive(Debug, Clone, Copy)]
pub enum HttpExchangePending {
    Waiting,
}

/// Platform-erased exchange task — `Box<dyn TaskIterator>` with shared types.
///
/// No platform types in `shared/`: each platform's `open_exchange` impl boxes its
/// own task. `Box<dyn TaskIterator<...> + Send>` implements `TaskIterator` via
/// valtron's blanket impl and works with `valtron::execute()`/`send()`.
pub type HttpExchangeClientTask = Box<
    dyn foundation_core::valtron::TaskIterator<
            Ready = HttpExchange,
            Pending = HttpExchangePending,
            Spawner = foundation_core::valtron::BoxedSendExecutionAction,
        > + Send,
>;
