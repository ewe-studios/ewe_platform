//! [`WebSocketConnector`] impl for [`FetchHttpClient`] (F51 Stage 6, wasm).
//!
//! WHY: F51 unifies the client — the same concrete type that serves HTTP on wasm
//! (`FetchHttpClient`, via `fetch`) also opens `WebSocket` connections through the
//! [`WebSocketConnector`] trait, returning the shared [`WebSocketClient`]. This is
//! the wasm half of the single, auto-switching `open_websocket` surface.
//!
//! WHAT: Implements `WebSocketConnector::open_websocket` by delegating to the
//! browser `WebSocket` bridge ([`super::browser::connect`]).
//!
//! HOW: `fetch` and `WebSocket` share nothing but the method surface — the
//! browser owns both. So this impl simply hands off to `browser::connect`,
//! which wires the browser socket into the shared client.

use crate::wasm::client::client::FetchHttpClient;
use crate::websocket::shared::client::{WebSocketClient, WebSocketConnectConfig};
use crate::websocket::shared::connector::WebSocketConnector;
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
}
