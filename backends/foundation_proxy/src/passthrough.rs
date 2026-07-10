//! Raw byte-level passthrough.
//!
//! WHY: Decision 14 §"Protocol inference" — a `tcp://` backend is proxied at the
//! byte level with no HTTP semantics (RDP, VNC, noVNC). The same primitive
//! relays a WebSocket connection once its `101` handshake is done: after the
//! upgrade everything is opaque frames. Both cases are "copy bytes both ways
//! until one side closes".
//!
//! WHAT: [`splice_bidirectional`] (the generic two-way copy used by the
//! WebSocket relay) and [`TcpPassthrough`] (a standalone raw TCP listener that
//! splices every accepted client to a fixed backend authority).
//!
//! HOW: One poll loop on a single thread moves data in both directions over
//! non-blocking sockets, sleeping briefly only when neither side had data. EOF
//! or a hard error on either side ends the splice.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_core::synca::OnSignal;

/// Copy bytes in both directions between `a` and `b` until one side closes.
///
/// WHAT: Reads whatever is available on each side and writes it to the other,
/// looping until an EOF (`Ok(0)`) or a non-`WouldBlock` error on either side.
///
/// # Preconditions
/// Both streams must already be in non-blocking mode; otherwise a read on an
/// idle side blocks the other direction. Callers own that setup because the two
/// stream types differ.
pub fn splice_bidirectional<A: Read + Write, B: Read + Write>(mut a: A, mut b: B) {
    let mut buf = [0u8; 16 * 1024];
    loop {
        let mut progressed = false;

        match a.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if b.write_all(&buf[..n]).and_then(|()| b.flush()).is_err() {
                    break;
                }
                progressed = true;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        match b.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if a.write_all(&buf[..n]).and_then(|()| a.flush()).is_err() {
                    break;
                }
                progressed = true;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        if !progressed {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// A raw TCP passthrough listener: every accepted client is spliced to
/// `backend_authority`.
///
/// WHY: The HTTP front end parses HTTP before dispatch, so a genuinely raw
/// protocol (`tcp://` service) cannot ride it — it needs its own listener that
/// hands the bytes straight to the backend.
#[derive(Debug)]
pub struct TcpPassthrough {
    local_addr: std::net::SocketAddr,
    shutdown: Arc<OnSignal>,
    accept_thread: Option<JoinHandle<()>>,
}

impl TcpPassthrough {
    /// Bind `listen_addr` and splice every connection to `backend_authority`.
    ///
    /// Returns immediately with a handle; the accept loop runs on its own
    /// thread until [`TcpPassthrough::shutdown`] (or drop).
    ///
    /// # Errors
    /// Returns the bind error if `listen_addr` cannot be bound.
    pub fn start(listen_addr: &str, backend_authority: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(listen_addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let shutdown = Arc::new(OnSignal::new());
        let backend = backend_authority.to_string();

        let loop_shutdown = Arc::clone(&shutdown);
        let accept_thread = std::thread::spawn(move || {
            accept_loop(&listener, &backend, &loop_shutdown);
        });

        Ok(Self {
            local_addr,
            shutdown,
            accept_thread: Some(accept_thread),
        })
    }

    /// The actual bound address (useful when binding to port 0).
    #[must_use]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.local_addr
    }

    /// Stop accepting and join the accept thread.
    pub fn shutdown(mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TcpPassthrough {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Accept loop for [`TcpPassthrough`]: one splice thread per client.
fn accept_loop(listener: &TcpListener, backend_authority: &str, shutdown: &Arc<OnSignal>) {
    loop {
        if shutdown.probe() {
            return;
        }
        match listener.accept() {
            Ok((client, _addr)) => {
                let backend = backend_authority.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = splice_client_to_backend(client, &backend) {
                        tracing::warn!(%backend, "tcp passthrough failed: {e}");
                    }
                });
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                tracing::error!("tcp passthrough accept error: {e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Connect to the backend and splice a single client connection to it.
fn splice_client_to_backend(client: TcpStream, backend_authority: &str) -> std::io::Result<()> {
    let backend = TcpStream::connect(backend_authority)?;
    client.set_nonblocking(true)?;
    backend.set_nonblocking(true)?;
    splice_bidirectional(client, backend);
    Ok(())
}
