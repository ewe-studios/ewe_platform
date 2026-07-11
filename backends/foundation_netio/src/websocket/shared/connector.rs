//! `WebSocketConnector` trait — segregated WebSocket open interface (F51 Stage 5).
//!
//! WHY: A segregated trait for opening WebSocket connections, implemented by the
//! concrete HTTP clients. HTTP and WebSocket are different lifecycles
//! (request→response vs persistent full-duplex); interface segregation keeps
//! each cohesive and lets HTTP-only consumers depend on `HttpClient` alone.
//!
//! WHAT: `WebSocketConnector` with `open_websocket(&self, url, headers) ->
//! Result<WebSocketConnection, WebSocketError>`. On native, the HTTP client
//! performs the Upgrade handshake over its pool/TLS. On wasm, the fetch client
//! delegates to the browser `WebSocket` API.
//!
//! HOW: Implementors wire the handshake through their connection infrastructure.

use crate::shared::http::SimpleHeaders;
use crate::websocket::native::connection::WebSocketConnection;
use crate::websocket::shared::error::WebSocketError;

/// Trait for opening WebSocket connections through a client.
///
/// WHY: Segregated from `HttpClient` — a WebSocket is a persistent full-duplex
/// channel, not a request→response exchange. HTTP-only consumers depend on
/// `HttpClient`; WebSocket consumers additionally require `WebSocketConnector`.
///
/// The same concrete client (`NativeHttpClient`, `FetchHttpClient`) implements
/// both traits, with the WebSocket opening reusing the client's connection
/// infrastructure (pool, TLS, DNS on native; browser WebSocket API on wasm).
pub trait WebSocketConnector: Send + Sync {
    /// Open a WebSocket connection to `url` with the given request headers.
    ///
    /// On native, the implementor performs an HTTP/1.1 `Upgrade` handshake over
    /// the client's connection pool and TLS infrastructure.
    ///
    /// On wasm, the implementor delegates to the browser `WebSocket` API.
    ///
    /// # Errors
    ///
    /// Returns `WebSocketError` if the connection or handshake fails.
    fn open_websocket(
        &self,
        url: &str,
        headers: SimpleHeaders,
    ) -> Result<WebSocketConnection, WebSocketError>;
}
