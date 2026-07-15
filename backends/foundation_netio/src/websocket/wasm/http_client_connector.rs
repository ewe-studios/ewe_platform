//! [`WebSocketConnector`] impl for [`FetchHttpClient`] (F51 Stage 6, wasm).
//!
//! WHY: F51 unifies the client — the same concrete type that serves HTTP on wasm
//! (`FetchHttpClient`, via `fetch`) also opens `WebSocket` connections through the
//! [`WebSocketConnector`] trait, returning the shared [`WebSocketClient`] or
//! [`WsExchangeTask`]. This is the wasm half of the single, auto-switching
//! `open_websocket` / `open_websocket_task` surface.
//!
//! WHAT: Implements both `WebSocketConnector` methods by delegating to the
//! browser `WebSocket` bridge ([`super::browser::connect`]). `open_websocket_task`
//! consumes the client's inner stream and wraps it in a `Stream → TaskStatus`
//! adapter so the transport pump gets a proper `TaskIterator`.
//!
//! HOW: `fetch` and `WebSocket` share nothing but the method surface — the
//! browser owns both. So this impl simply hands off to `browser::connect`,
//! which wires the browser socket into the shared client.

use crate::wasm::client::client::FetchHttpClient;
use crate::websocket::shared::client::{MessageDelivery, WebSocketClient, WebSocketConnectConfig};
use crate::websocket::shared::connector::{WebSocketConnector, WsExchangeTask};
use crate::websocket::shared::error::WebSocketError;

impl WebSocketConnector for FetchHttpClient {
    fn open_websocket(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError> {
        let (client, _delivery) = super::browser::connect(url, &config)?;
        Ok(client)
    }

    fn open_websocket_task(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<(WsExchangeTask, MessageDelivery), WebSocketError> {
        let (client, delivery) = super::browser::connect(url, &config)?;
        let stream = client.into_parts().0;
        let task = super::browser::boxed_stream_to_task(stream);
        Ok((task, delivery))
    }
}
