//! `Connector` impl — opens HTTP connections through the WireGuard mesh (spec-55, F11).
//!
//! WHY: `HttpConnectionPool` has a `Connector` trait with `connect(host, port, timeout)`.
//! `OverlayConnector` implements it by calling `WgHandle::overlay_connect()` — so every
//! netio HTTP consumer (ConnectRPC, gRPC, NativeHttpClient) works over the mesh with
//! one `.with_connector(Arc::new(OverlayConnector::new(handle)))` call.
//!
//! WHAT: `OverlayConnector` holds an `Arc<WgHandle>` (cheap clone). `connect()` parses
//! the host as an overlay IP (10.x.y.z), calls `handle.overlay_connect(ip, port)`.

use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use foundation_netio::native::connection::Connector;
use foundation_netio::netcap::Connection;

use super::WgHandle;

/// Connector that opens connections through the WireGuard mesh.
///
/// Usage:
/// ```ignore
/// let pool = HttpConnectionPool::new(pool, resolver)
///     .with_connector(Arc::new(OverlayConnector::new(Arc::new(wg_handle))));
/// ```
pub struct OverlayConnector {
    handle: Arc<WgHandle>,
}

impl OverlayConnector {
    /// Wrap a `WgHandle` as a `Connector`.
    #[must_use]
    pub fn new(handle: Arc<WgHandle>) -> Self {
        Self { handle }
    }
}

impl Connector for OverlayConnector {
    fn connect(
        &self,
        host: &str,
        port: u16,
        timeout: Option<Duration>,
    ) -> io::Result<Connection> {
        // Parse the host as an overlay IP. WireGuard overlay addresses
        // are in the 10.0.0.0/8 range.
        let ip: IpAddr = host
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, format!("not an overlay IP: {host}")))?;
        // The Connector contract returns an established connection, so drive the
        // overlay handshake to completion (default 8s when the caller sets none).
        let timeout = timeout.unwrap_or_else(|| Duration::from_secs(8));
        tracing::debug!(%ip, port, ?timeout, "OverlayConnector: dialing overlay peer");
        let res = self.handle.overlay_connect_timeout(ip, port, timeout);
        tracing::debug!(%ip, port, ok = res.is_ok(), "OverlayConnector: overlay dial result");
        res
    }
}
