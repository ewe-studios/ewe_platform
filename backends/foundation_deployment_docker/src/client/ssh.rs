//! SSH transport for `DockerClient` (`DOCKER_HOST=ssh://user@host`).
//!
//! WHY: SSH is Docker's port-less remote transport — no exposed TCP, no cert
//! management. It works by SSHing in and running `docker system dial-stdio`,
//! which bridges the command's stdin/stdout to the remote `/var/run/docker.sock`.
//! The channel *is* the socket; the client speaks the ordinary Docker HTTP API
//! over it.
//!
//! WHAT: [`SshConnector`] implements `foundation_netio`'s [`Connector`] seam —
//! the same one the WireGuard overlay uses (F11). Each fresh connection dials a
//! new SSH channel via a session-caching [`Dialer`] and wraps it as
//! [`SshOverlay`], a [`OverlayReadWrite`] presented to the HTTP client as a
//! `Connection::Overlay`.
//!
//! HOW: The channel is blocking (like a Unix socket), so the HTTP request task
//! reads/writes it directly — no async bridge. The overlay reports
//! `should_pool() == false`: a `dial-stdio` bridge is one-shot, so the pool must
//! never hand a spent channel to a later request.

use std::io::{Read, Write};
use std::sync::Arc;

use foundation_netio::netcap::connection::{
    ConnWaker, Connection, Connector, OverlayReadWrite,
};
use foundation_sshkit::{ChannelStream, Dialer, Host};

/// The remote command whose stdio is the Docker socket.
const DIAL_STDIO: &str = "docker system dial-stdio";

/// A blocking SSH exec channel presented to the HTTP client as an overlay stream.
///
/// Wraps [`foundation_sshkit::ChannelStream`] (which lacks `Debug`) and supplies
/// the `OverlayReadWrite` surface. The readiness wakers are no-ops — the channel
/// is blocking, so the request task never parks on `WouldBlock`.
struct SshOverlay(ChannelStream);

impl std::fmt::Debug for SshOverlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SshOverlay(docker system dial-stdio)")
    }
}

impl Read for SshOverlay {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for SshOverlay {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl OverlayReadWrite for SshOverlay {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite> {
        Box::new(SshOverlay(self.0.clone()))
    }

    /// The SSH channel has no meaningful socket address — report a placeholder.
    /// Only used as connection metadata (`Connection::stream_addr`).
    fn local_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], 0))
    }

    fn peer_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], 0))
    }

    // Blocking transport — no readiness driver, so wakers are never fired.
    fn set_read_waker(&self, _waker: ConnWaker) {}
    fn set_write_waker(&self, _waker: ConnWaker) {}

    /// `docker system dial-stdio` is a one-shot bridge to the remote socket — the
    /// channel must never be pooled and reused for a later request.
    fn should_pool(&self) -> bool {
        false
    }
}

/// A [`Connector`] that dials `docker system dial-stdio` over SSH per connection.
///
/// Holds a session-caching [`Dialer`] so sequential requests reuse one SSH
/// handshake while each gets its own fresh channel.
pub struct SshConnector {
    dialer: Arc<Dialer>,
}

impl SshConnector {
    /// Build a connector for `host` (an already-parsed sshkit [`Host`]).
    #[must_use]
    pub fn new(host: Host) -> Self {
        Self {
            dialer: Arc::new(Dialer::new(host)),
        }
    }
}

impl Connector for SshConnector {
    /// `host`/`port`/`timeout` are ignored: the SSH target is fixed by the
    /// `Dialer`, and the HTTP URL host is always `localhost` (the remote's own
    /// docker socket). Opens a fresh `dial-stdio` channel and wraps it.
    fn connect(
        &self,
        _host: &str,
        _port: u16,
        _timeout: Option<std::time::Duration>,
    ) -> std::io::Result<Connection> {
        let channel = self.dialer.dial(DIAL_STDIO).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, format!("ssh dial: {e}"))
        })?;
        Ok(Connection::Overlay(Box::new(SshOverlay(channel))))
    }
}
