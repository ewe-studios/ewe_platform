//! Cross-platform HTTP client trait.
//!
//! WHY: AI providers and other HTTP consumers need a single interface that works
//! on both native (TCP sockets) and wasm32 (browser `fetch` / CF Workers).
//! By coding against `dyn HttpClient`, callers become platform-agnostic.
//!
//! WHAT: Defines the `HttpClient` trait with four methods:
//! - `send_async()` / `send_sse_async()` — async (canonical, holds real logic)
//! - `send()` / `send_sse()` — sync (wraps async via valtron, providers call these)
//!
//! HOW: Trait is `#[async_trait]` and object-safe (`Arc<dyn HttpClient>`).
//! Native impl overrides sync methods with direct sync calls (no async overhead).
//! Wasm impl relies on the default sync→async bridge via `valtron::run_future()`.
//! The outside world doesn't need to know about the async/sync bridging.

use foundation_core::valtron::Stream;

use super::request::PreparedRequest;
use crate::event_source::ParseResult;
use crate::simple_http::shared::{HttpClientError, SendSafeBody, SimpleResponse};

/// Progress indicator for SSE streams.
///
/// Maps from the native `ReconnectingProgress` to a platform-neutral enum
/// so consumers don't depend on native reconnection internals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SseProgress {
    Connecting,
    Reading,
    Reconnecting,
}

/// Boxed iterator yielding SSE events with progress updates.
///
/// Each item is a `Stream<ParseResult, SseProgress>` — either a parsed SSE
/// event (`Stream::Next`), a progress signal (`Stream::Pending`), or a
/// flow-control variant (`Stream::Wait`, `Stream::Init`, etc.) driven by
/// valtron's executor model.
pub type BoxedSseIterator = Box<dyn Iterator<Item = Stream<ParseResult, SseProgress>> + Send>;

/// Platform-agnostic HTTP client with both async and sync surfaces.
///
/// Async methods (`send_async`, `send_sse_async`) hold the canonical logic.
/// Sync methods (`send`, `send_sse`) wrap async via `valtron::run_future()`
/// by default — native overrides them with direct sync implementations so
/// providers and other callers just call `client.send(req)` without knowing
/// about async internals.
///
/// Store as `Arc<dyn HttpClient>` for dynamic dispatch.
#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    /// Send an HTTP request asynchronously — canonical implementation.
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;

    /// Send an SSE request asynchronously — canonical implementation.
    async fn send_sse_async(
        &self,
        req: PreparedRequest,
    ) -> Result<BoxedSseIterator, HttpClientError>;

    /// Send an HTTP request synchronously.
    ///
    /// Default bridges to `send_async` via `valtron::run_future()`.
    /// Native overrides this with a direct sync path (no async overhead).
    fn send(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;

    /// Send an SSE request synchronously.
    ///
    /// Default bridges to `send_sse_async` via `valtron::run_future()`.
    /// Native overrides this with a direct sync path.
    fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError>;
}
