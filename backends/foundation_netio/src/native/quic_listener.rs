//! QUIC listener for netcap (F35 — D12 §9).
//!
//! WHY: The `Listener` enum's blocking `accept()` API doesn't fit QUIC's
//! non-blocking `take_accepted()`. Rather than forcing QUIC into the existing
//! shape, the QUIC listener is a separate type that the H3 server handles.
//!
//! WHAT: [`QuicListener`] owns a [`QuicDriver`] and pumps it in `accept()`,
//! returning raw [`QuinnConnection`]s. Created via [`QuicBind`] on
//! [`ConfigListenAddr`](super::connection::ConfigListenAddr).
//!
//! HOW: The caller drives H3 on each accepted connection via
//! `H3Connection::new(conn)`, spawning tasks per H3 request.

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::io;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::net::SocketAddr;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::sync::Arc;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::time::{Duration, Instant};

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_core::valtron::TaskIterator;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use crate::quic::{QuicDriver, QuinnConnection, ServerConfig};

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub struct QuicListener {
    driver: QuicDriver,
    local_addr: SocketAddr,
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
impl QuicListener {
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Block until a new QUIC connection arrives. Pumps the driver with a 1 ms
    /// sleep between polls.
    ///
    /// # Errors
    /// `TimedOut` if nothing arrives within 30 seconds.
    pub fn accept(&mut self) -> io::Result<QuinnConnection> {
        self.accept_timeout(Duration::from_secs(30))
    }

    pub fn accept_timeout(&mut self, timeout: Duration) -> io::Result<QuinnConnection> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(conn) = self.driver.take_accepted() {
                return Ok(conn);
            }
            let _ = self.driver.next_status();
            if Instant::now() > deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "QUIC listener: no connection within timeout",
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Extension trait: bind a QUIC listener from a [`ConfigListenAddr`](super::connection::ConfigListenAddr).
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub trait QuicBind {
    fn bind_quic(self, server_config: Arc<ServerConfig>) -> io::Result<QuicListener>;
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
impl QuicBind for crate::native::connection::ConfigListenAddr {
    fn bind_quic(self, server_config: Arc<ServerConfig>) -> io::Result<QuicListener> {
        let addr = match &self {
            crate::native::connection::ConfigListenAddr::IP(addrs) => {
                addrs.first().copied().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "no address in ConfigListenAddr")
                })?
            }
            #[cfg(unix)]
            crate::native::connection::ConfigListenAddr::Unix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "QUIC requires an IP address",
                ))
            }
        };

        let quic_config = Arc::try_unwrap(server_config).unwrap_or_else(|arc| (*arc).clone());
        let driver = QuicDriver::server(addr, quic_config)?;
        let local_addr = driver.local_addr()?;

        Ok(QuicListener { driver, local_addr })
    }
}
