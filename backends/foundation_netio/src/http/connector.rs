//! Connection connector abstraction (spec-55, F11).
//!
//! WHY: `HttpConnectionPool` hardcodes DNS resolution + `TcpStream::connect()`.
//! This trait lets different transports plug in — kernel TCP, WireGuard overlay,
//! Unix sockets — without changing the HTTP stack.
//!
//! WHAT: `Connector::connect(host, port, timeout) -> Connection`. When set on
//! the pool via `with_connector`, it replaces the DNS+TCP path in `create_http_connection`.
//!
//! HOW: Pool still does its normal DNS+pooled lookup first. Only when creating a
//! fresh connection (fallback path) does it use the connector if one is set.

use std::io;
use std::time::Duration;

use crate::netcap::Connection;

/// Opens a `Connection` for a given host:port.
///
/// Implementors: `OverlayConnector` (WireGuard mesh — in `foundation_wireguard`).
pub trait Connector: Send + Sync + 'static {
    /// Open a connection to `host:port` with an optional timeout.
    fn connect(
        &self,
        host: &str,
        port: u16,
        timeout: Option<Duration>,
    ) -> io::Result<Connection>;
}
