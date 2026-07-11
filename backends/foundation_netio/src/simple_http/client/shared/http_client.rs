//! Cross-platform HTTP client trait.
//!
//! WHY: AI providers and other HTTP consumers need a single interface that works
//! on both native (TCP sockets) and wasm32 (browser `fetch` / CF Workers).
//! By coding against `dyn HttpClient`, callers become platform-agnostic.
//!
//! WHAT: Defines the `HttpClient` trait with four methods:
//! - `send_async()` — returns a future (async callers `.await` it)
//! - `send_sse_async()` — returns a future stream (async callers `.next().await`)
//! - `send()` — returns a result directly (sync callers)
//! - `send_sse()` — returns a sync iterator (sync callers `for item in iter`)
//!
//! HOW: Trait is `#[async_trait]` and object-safe (`Arc<dyn HttpClient>`).
//! Native sync methods call `SimpleHttpClient` directly.
//! Native async methods use `ClientRequest::send_async()` and `into_future_stream()`.
//! Wasm async methods use `fetch()`. Wasm sync wraps async via `run_future()`.

use std::pin::Pin;

use foundation_core::valtron::Stream;

use super::request::PreparedRequest;
use super::request_task::HttpExchangeClientTask;
use crate::event_source::ParseResult;
use crate::simple_http::shared::{HttpClientError, SendSafeBody, SimpleResponse};

/// Progress indicator for SSE streams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SseProgress {
    Connecting,
    Reading,
    Reconnecting,
}

/// Boxed sync iterator yielding SSE events — for sync callers.
pub type BoxedSseIterator = Box<dyn Iterator<Item = Stream<ParseResult, SseProgress>> + Send>;

/// Boxed async stream yielding SSE events — for async callers.
pub type BoxedSseFutureStream = Pin<
    Box<dyn futures_core::Stream<Item = Stream<ParseResult, SseProgress>> + Send>,
>;

/// Platform-agnostic HTTP client with both async and sync surfaces.
///
/// Async methods return async types (futures, future streams).
/// Sync methods return sync types (results, iterators).
/// Each platform implements both properly — no lazy delegation.
///
/// Store as `Arc<dyn HttpClient>` for dynamic dispatch.
#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    /// Send an HTTP request asynchronously.
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;

    /// Send an SSE request asynchronously — returns a future stream.
    async fn send_sse_async(
        &self,
        req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError>;

    /// Send an HTTP request synchronously.
    fn send(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;

    /// Send an SSE request synchronously — returns a sync iterator.
    fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError>;

    /// Build a valtron task that drives one request/response exchange, yielding
    /// [`HttpExchange`](super::request_task::HttpExchange) items (`Head`,
    /// `BodyChunk`, `Failed`). The caller spawns it with `valtron::send()` or
    /// composes it with split combinators.
    ///
    /// WHY: Lets the transport pump (`H1Transport`, WASM fetch pump) be
    /// platform-agnostic — the client owns the pool, config, and resolver, so
    /// the pump never names a concrete transport type.
    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask;
}
