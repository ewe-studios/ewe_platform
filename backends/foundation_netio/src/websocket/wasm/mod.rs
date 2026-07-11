//! Wasm (browser) WebSocket support (F51 Stage 6).
//!
//! The browser `WebSocket` bridge and the [`FetchHttpClient`](crate::wasm::client::client::FetchHttpClient)
//! [`WebSocketConnector`](crate::websocket::shared::connector::WebSocketConnector)
//! impl — the wasm half of the unified, cross-platform WebSocket client.

pub mod browser;
pub mod http_client_connector;

pub use browser::connect;
