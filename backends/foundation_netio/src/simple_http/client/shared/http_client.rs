//! Cross-platform HTTP client trait.
//!
//! WHY: AI providers and other HTTP consumers need a single interface that works
//! on both native (TCP sockets) and wasm32 (browser `fetch` / CF Workers).
//! By coding against `dyn HttpClient`, callers become platform-agnostic.
//!
//! WHAT: Defines the async `HttpClient` trait with two methods:
//! - `send()` — full request/response cycle returning `SimpleResponse<SendSafeBody>`
//! - `send_sse()` — SSE streaming returning a boxed iterator of parsed events
//!
//! HOW: Trait is `#[async_trait]` and object-safe (`Arc<dyn HttpClient>`).
//! Native impl wraps `SimpleHttpClient` + `ReconnectingEventSourceTask`.
//! Wasm impl wraps `web_sys::fetch` + `ReadableStream` → `SseParser` bridge.
//! Sync callers drive async methods through `valtron::run_future()`.

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

/// Platform-agnostic async HTTP client.
///
/// Implementations exist for native (`NativeHttpClient`) and wasm32
/// (`FetchHttpClient`). Store as `Arc<dyn HttpClient>` for dynamic dispatch.
///
/// Uses `PreparedRequest` (shared, cross-platform) for input and
/// `SimpleResponse<SendSafeBody>` for output — both types support the full
/// range of body variants including streaming (`SendSafeBody::Stream`,
/// `SseStream`, `ChunkedStream`, etc.).
#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    /// Send an HTTP request and return the full response.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the request fails (DNS, connection,
    /// timeout, protocol error, etc.).
    async fn send(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;

    /// Send an HTTP request expecting an SSE (`text/event-stream`) response.
    ///
    /// Returns a boxed iterator that yields parsed SSE events. On native this
    /// wraps `ReconnectingEventSourceTask`; on wasm it reads `ReadableStream`
    /// chunks through `SseParser`.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the connection or initial handshake fails.
    async fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError>;
}
