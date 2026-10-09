//! Native-only ConnectRPC modules (non-wasm).
//!
//! The server connection-owner adapters and the HTTP/2 + HTTP/3 client
//! transports live here because they drive native-only substrate: TCP
//! `RawStream` (server), `foundation_netio::http2` (h2), and
//! `foundation_netio::quic`/`http3` (h3). The cross-platform seam and the
//! HTTP/1.1 + WebSocket transports (which ride `foundation_netio`'s
//! platform-selecting client) stay under [`crate::shared`].

/// Native server connection-owner adapter (foundation_http `Serve`).
pub mod server;

/// HTTP/2 server connection-owner adapter (foundation_http `H2Serve`).
pub mod h2_serve;

/// HTTP/3 server connection-owner adapter (foundation_http `H3Serve`, F35).
#[cfg(feature = "h3")]
pub mod h3_serve;

/// Native client transports (HTTP/2 over h2c, HTTP/3 over QUIC).
pub mod transport;
