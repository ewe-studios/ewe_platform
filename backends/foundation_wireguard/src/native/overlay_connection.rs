//! `OverlayReadWrite` impl — bridges `OverlayStream` into `Connection::Overlay` (spec-55, F11).
//!
//! WHY: ConnectRPC/HTTP/any-netio-consumer works over the WireGuard mesh through
//! one `Connection`. The `OverlayReadWrite` trait in `foundation_netio` provides the
//! interface; this module provides the impl wrapping a smoltcp `OverlayStream`.
//!
//! WHAT: [`OverlayConnection`] wraps a [`OverlayStream`] and implements
//! `std::io::Read + Write + OverlayReadWrite`. Constructed by
//! [`WgHandle::overlay_connect`] and fed to `Connection::Overlay(...)`.
//!
//! HOW: `OverlayStream::read(&self, ...)` and `write(&self, ...)` take `&self`
//! (the smoltcp socket is behind `Arc<Mutex<>>`), so the `Read`/`Write` impls on
//! `OverlayConnection` just delegate through — no locking needed at this level.

use std::io;

use foundation_nativeapis::dataplane::netstack::OverlayStream;
use foundation_netio::native::connection::OverlayReadWrite;

/// Wraps a smoltcp `OverlayStream` as an `OverlayReadWrite` trait object so it
/// can be boxed into `Connection::Overlay(Box<dyn OverlayReadWrite>)`.
#[derive(Debug, Clone)]
pub struct OverlayConnection {
    pub stream: OverlayStream,
}

impl io::Read for OverlayConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl io::Write for OverlayConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(()) // smoltcp flushes on each send
    }
}

impl OverlayReadWrite for OverlayConnection {
    fn clone_box(&self) -> Box<dyn OverlayReadWrite> {
        // OverlayStream derives Clone — cheap Arc ref-count bump.
        Box::new(Self {
            stream: self.stream.clone(),
        })
    }
}
