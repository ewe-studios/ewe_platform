//! [`WebSocketConnector`] impl for [`NativeHttpClient`] (F51 Stage 6).
//!
//! WHY: F51 unifies the client — the same concrete type that serves HTTP also
//! opens `WebSocket` connections through the [`WebSocketConnector`] trait, and
//! returns the cross-platform [`WebSocketClient`] rather than a native-only
//! connection type. That is what lets a caller write one `open_websocket` call
//! that compiles and works on both native and wasm.
//!
//! WHAT: Implements `WebSocketConnector::open_websocket` by driving a native
//! WebSocket task over the client's own resolver (DNS → TCP → TLS → HTTP/1.1
//! Upgrade), honouring the shared [`WebSocketConnectConfig`] (subprotocols,
//! headers, timeouts, and — native-only — reconnection).
//!
//! HOW: Delegates to [`WebSocketClient::connect_with_config`], which builds the
//! single-shot or reconnecting `WsTask`, spawns it on valtron, and boxes the
//! driven stream into the shared client. A WebSocket upgrade permanently
//! consumes its connection (it is no longer a reusable HTTP connection), so the
//! resolver path — not the HTTP connection pool — is the correct basis, and it
//! also carries reconnection support the pool path lacks.

use crate::http::NativeHttpClient;
use crate::shared::client::DnsResolver;
use crate::websocket::shared::client::{WebSocketClient, WebSocketConnectConfig};
use crate::websocket::shared::connector::WebSocketConnector;
use crate::websocket::shared::error::WebSocketError;

impl<R: DnsResolver + Clone + Send + Sync + 'static> WebSocketConnector for NativeHttpClient<R> {
    fn open_websocket(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError> {
        let (client, _delivery) =
            WebSocketClient::connect_with_config(self.resolver().clone(), url, &config)?;
        Ok(client)
    }
}
