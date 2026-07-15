//! Cross-platform ConnectRPC modules — compiled on every target (native + wasm).
//!
//! Everything here is free of native-only substrate (no TCP `RawStream`, no
//! `foundation_netio::http2`/`quic`). The native server + h2/h3 transports live
//! under [`crate::native`]; wasm reuses these shared modules with
//! `foundation_netio`'s platform-selecting client/websocket wrappers underneath.

pub mod client;
pub mod codec;
pub mod compression;
pub mod context;
pub mod envelope;
pub mod error;
pub mod error_writer;
pub mod interceptor;
#[cfg(feature = "auth")]
pub mod auth;
pub mod message;
pub mod protocol;
pub mod router;
pub mod transport;
