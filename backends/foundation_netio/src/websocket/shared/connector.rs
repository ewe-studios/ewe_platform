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
//! `WebSocketConnector::open_websocket_task` returns a boxed
//! [`WsExchangeTask`] + [`MessageDelivery`](client::MessageDelivery), mirroring
//! `HttpClient::open_exchange()` — the transport pump spawns the task on its own
//! pool and drives it with `split_collector_map`. The caller decides when to
//! execute; the connector hides all platform differences behind the boxed type.
//!
//! HOW: Native performs the HTTP/1.1 Upgrade over its own dial/TLS and drives a
//! valtron task; wasm hands off to the browser `WebSocket` API. Both box their
//! inbound stream into the shared [`WebSocketClient`] or [`WsExchangeTask`].

use std::sync::Arc;

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator};

use crate::shared::client::http_client::HttpClient;
use crate::websocket::shared::client::{MessageDelivery, WebSocketClient, WebSocketConnectConfig};
use crate::websocket::shared::error::WebSocketError;
use crate::websocket::shared::message::WebSocketMessage;

/// A client that implements both `HttpClient` and `WebSocketConnector`.
///
/// The platform's concrete client (`NativeHttpClient` or `FetchHttpClient`)
/// implements both traits, so an `Arc<dyn FullHttpClient>` serves as the
/// single shared handle for the HTTP transport, the WS transport, and
/// everything else that needs a client.
pub trait FullHttpClient: HttpClient + WebSocketConnector {}
impl<T: HttpClient + WebSocketConnector> FullHttpClient for T {}

/// Shared, clonable handle to a platform client that does HTTP + WebSocket.
pub type DynHttpClient = Arc<dyn FullHttpClient>;

/// Boxed, platform-erased WebSocket task — `Box<dyn TaskIterator>` with shared
/// Ready/Pending/Spawner types. Mirrors
/// [`HttpExchangeClientTask`](crate::shared::client::request_task::HttpExchangeClientTask).
///
/// No platform types in the signature: native spawns a `WsTask` and maps
/// `WsPending → WsProgress`; wasm wraps the browser `WebSocket` bridge.
/// Both produce the same boxed type so the transport pump code is
/// platform-agnostic.
pub type WsExchangeTask = Box<
    dyn TaskIterator<
        Ready = Result<WebSocketMessage, WebSocketError>,
        Pending = crate::websocket::shared::client::WsProgress,
        Spawner = BoxedSendExecutionAction,
    > + Send,
>;

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

    /// Open a WebSocket connection returning a platform-erased [`WsExchangeTask`]
    /// + [`MessageDelivery`], mirroring `HttpClient::open_exchange()`.
    ///
    /// The returned task is **not spawned** — the caller decides when to
    /// `valtron::execute()` it on their own pool. This is the cross-platform
    /// entry point for the transport pump.
    ///
    /// On native, spawns the single-shot or reconnecting task via
    /// `valtron::execute` and boxes the driven iterator. On wasm, wraps the
    /// browser `WebSocket` bridge in a `Stream → TaskStatus` adapter and boxes it.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails.
    fn open_websocket_task(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<(WsExchangeTask, MessageDelivery), WebSocketError>;
}
