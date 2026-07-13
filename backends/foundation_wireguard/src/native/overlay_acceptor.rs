//! `Acceptor` impl — bridges `OverlayListener` into `HttpServer` (spec-55, F11).
//!
//! WHY: `HttpServer` takes a generic `Acceptor` trait (defined in `foundation_netio`).
//! `OverlayAcceptor` wraps a smoltcp `OverlayListener` so `HttpServer` can serve
//! on a WireGuard overlay IP with zero transport changes — `accept_connection`
//! returns `Connection::Overlay(...)` just like TCP returns `Connection::Tcp(...)`.
//!
//! WHAT: [`OverlayAcceptor`] wraps an [`OverlayListener`] + overlay IP.
//!
//! HOW: `OverlayListener::accept(&mut self)` returns `OverlayStream`. We wrap it
//! in an `OverlayReadWrite` trait object and return `Connection::Overlay(Box<dyn O>)`.

use std::io;
use std::net::SocketAddr;

use foundation_nativeapis::dataplane::netstack::{OverlayListener, OverlayStream};
use foundation_netio::native::connection::{Acceptor, Connection, OverlayReadWrite};

/// Wraps an `OverlayListener` as an `Acceptor` so `HttpServer` can serve on a
/// WireGuard overlay IP.
pub struct OverlayAcceptor {
    listener: OverlayListener,
    local: SocketAddr,
}

impl OverlayAcceptor {
    /// Create from an `OverlayListener` (obtained via `WgHandle::tcp_listen`).
    pub fn new(listener: OverlayListener, local_ip: std::net::IpAddr, port: u16) -> Self {
        Self {
            listener,
            local: SocketAddr::new(local_ip, port),
        }
    }
}

impl Acceptor for OverlayAcceptor {
    fn accept_connection(&mut self) -> io::Result<(Connection, SocketAddr)> {
        let stream = self.listener.accept()?;
        let peer = stream.peer_addr();
        let conn = Connection::Overlay(Box::new(OverlayConn { stream }));
        Ok((conn, peer))
    }

    fn set_nonblocking(&self, _: bool) -> io::Result<()> {
        Ok(()) // smoltcp sockets are non-blocking by construction
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }
}

// ── OverlayReadWrite impl ────────────────────────────────────────────

struct OverlayConn {
    stream: OverlayStream,
}

impl io::Read for OverlayConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl io::Write for OverlayConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl OverlayReadWrite for OverlayConn {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite> {
        Box::new(OverlayConn {
            stream: self.stream.clone(),
        })
    }
}

impl std::fmt::Debug for OverlayConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayConn").finish_non_exhaustive()
    }
}

// Safety: OverlayStream is Send+Sync (Arc<Mutex<>> internally).
unsafe impl Send for OverlayConn {}
unsafe impl Sync for OverlayConn {}
