//! `WebSocketConnector` trait — segregated WebSocket open interface (F51 Stage 5).
//!
//! WHY: A segregated trait for opening WebSocket connections, implemented by the
//! concrete HTTP clients. HTTP and WebSocket are different lifecycles
//! (request→response vs persistent full-duplex); interface segregation keeps
//! each cohesive and lets HTTP-only consumers depend on `HttpClient` alone.
//!
//! WHAT: `WebSocketConnector::open_websocket(&self, url, config) ->
//! Result<WebSocketClient, WebSocketError>`. This is the **single, auto-switching
//! surface**: the same call yields a native or a browser-backed
//! [`WebSocketClient`] depending only on which concrete client (`NativeHttpClient`
//! vs `FetchHttpClient`) the caller holds — the returned type and the
//! [`WebSocketConnectConfig`] are identical on both targets.
//!
//! HOW: Native performs the HTTP/1.1 Upgrade over its own dial/TLS and drives a
//! valtron task; wasm hands off to the browser `WebSocket` API. Both box their
//! inbound stream into the shared [`WebSocketClient`].

use crate::websocket::shared::client::{WebSocketClient, WebSocketConnectConfig};
use crate::websocket::shared::error::WebSocketError;

/// Trait for opening WebSocket connections through a client.
///
/// WHY: Segregated from `HttpClient` — a WebSocket is a persistent full-duplex
/// channel, not a request→response exchange. HTTP-only consumers depend on
/// `HttpClient`; WebSocket consumers additionally require `WebSocketConnector`.
///
/// The same concrete client (`NativeHttpClient`, `FetchHttpClient`) implements
/// both traits, with the WebSocket opening reusing the client's connection
/// infrastructure (dial, TLS, DNS on native; browser WebSocket API on wasm).
pub trait WebSocketConnector: Send + Sync {
    /// Open a WebSocket connection to `url` with the given [`WebSocketConnectConfig`].
    ///
    /// On native, the implementor performs an HTTP/1.1 `Upgrade` handshake over
    /// the client's own resolver/TLS, optionally layering reconnection.
    ///
    /// On wasm, the implementor delegates to the browser `WebSocket` API. Fields
    /// the browser owns (reconnection lifecycle) are best-effort — see
    /// [`WebSocketConnectConfig`].
    ///
    /// Returns a cross-platform [`WebSocketClient`]: iterate `client.messages()`
    /// for inbound messages, and use `client.delivery()` to send.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the connection or handshake fails.
    fn open_websocket(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError>;
}
