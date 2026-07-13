//! `Acceptor` impl — bridges `OverlayListener` into `HttpServer` (spec-55, F11).
//!
//! WHY: `HttpServer` takes a generic `Acceptor` trait (defined in `foundation_netio`).
//! `OverlayAcceptor` wraps a smoltcp `OverlayListener` so `HttpServer` can serve
//! on a WireGuard overlay IP with zero transport changes — `accept_connection`
//! returns `Connection::Overlay(...)` just like TCP returns `Connection::Tcp(...)`.
//!
//! WHAT: [`OverlayAcceptor`] wraps an [`OverlayListener`] + overlay IP behind a
//! `Mutex` for interior mutability (`Acceptor::accept_connection` takes `&self`).
//!
//! HOW: `OverlayListener::accept(&mut self)` returns `OverlayStream`. We lock the
//! inner `Mutex<OverlayListener>` on each accept call and return the stream wrapped
//! in `Connection::Overlay(Box<dyn OverlayReadWrite>)`.

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use foundation_nativeapis::dataplane::netstack::{OverlayListener, OverlayStream};
use foundation_netio::native::connection::{Acceptor, Connection, OverlayReadWrite};

/// Wraps an `OverlayListener` behind a `Mutex` so it can implement `Acceptor`
/// (which takes `&self`).
pub struct OverlayAcceptor {
    listener: Arc<Mutex<OverlayListener>>,
    local: SocketAddr,
}

impl OverlayAcceptor {
    /// Create from an `OverlayListener` (obtained via `WgHandle::tcp_listen`).
    pub fn new(listener: OverlayListener, local_ip: std::net::IpAddr, port: u16) -> Self {
        Self {
            listener: Arc::new(Mutex::new(listener)),
            local: SocketAddr::new(local_ip, port),
        }
    }
}

// Clone is needed because HttpServer's serve_loop takes `&impl Acceptor`.
// Arc already provides Clone; Mutex does the interior mutability.
impl Clone for OverlayAcceptor {
    fn clone(&self) -> Self {
        Self {
            listener: Arc::clone(&self.listener),
            local: self.local,
        }
    }
}

impl Acceptor for OverlayAcceptor {
    fn accept_connection(&self) -> io::Result<(Connection, SocketAddr)> {
        let mut guard = self.listener.lock().expect("OverlayAcceptor::accept_connection: Mutex poisoned");
        let stream = guard.accept()?;
        let peer = stream.peer_addr();
        drop(guard);
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

// Safety: OverlayStream is Send+Sync (Arc<Mutex<Inner>> internally).
unsafe impl Send for OverlayConn {}
unsafe impl Sync for OverlayConn {}
