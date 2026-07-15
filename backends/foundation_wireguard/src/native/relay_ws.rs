//! WebSocket relay acceptor — browser peer entry point (spec-55, F07).
//!
//! WHY: Browser peers have no UDP — they connect to the mesh through a
//! WebSocket relay. The native relay-capable member must accept their
//! WebSocket connections, register relay sessions, and forward encrypted
//! WG frames between the browser and its peers.
//!
//! WHAT: [`RelayWsAcceptor`] wraps a `TcpListener`, performs the WebSocket
//! upgrade handshake, and feeds inbound WS frames into the relay server's
//! forward path. Outbound relay frames (from the relay server) are written
//! back as WS frames to the browser.
//!
//! HOW: WebSocket handshake uses the existing `foundation_netio::websocket`
//! infrastructure (`compute_accept_key`, `WebSocketFrame`, `Opcode`).
//! Each WS connection is a relay session. The browser identifies itself by
//! the relay frame content; frames carry the same `RelayFrame` format as
//! the UDP relay path.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use foundation_netio::websocket::shared::frame::{self, Opcode, WebSocketFrame};
use foundation_netio::websocket::shared::handshake::compute_accept_key;

use crate::shared::membership::PeerId;
use crate::shared::relay::RelayFrame;

use super::relay::RelayServer;

/// Read the HTTP upgrade request and extract the WebSocket key.
fn parse_ws_key(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    for line in req.lines() {
        if let Some(val) = line.strip_prefix("Sec-WebSocket-Key:") {
            return Ok(val.trim().to_string());
        }
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, "no Sec-WebSocket-Key"))
}

/// Send the 101 Switching Protocols response.
fn send_ws_upgrade(stream: &mut TcpStream, accept_key: &str) -> io::Result<()> {
    write!(stream, "HTTP/1.1 101 Switching Protocols\r\n")?;
    write!(stream, "Upgrade: websocket\r\n")?;
    write!(stream, "Connection: Upgrade\r\n")?;
    write!(stream, "Sec-WebSocket-Accept: {accept_key}\r\n")?;
    write!(stream, "\r\n")?;
    stream.flush()
}

/// Read one WebSocket frame using `foundation_netio`'s frame decoder.
fn read_ws_frame(stream: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    match WebSocketFrame::decode(stream) {
        Ok(frame) => {
            if frame.opcode == Opcode::Close {
                return Ok(None);
            }
            Ok(Some(frame.payload))
        }
        Err(e) => {
            // Map websocket errors to io errors for compatibility.
            Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
        }
    }
}

/// Write a WebSocket binary frame using `foundation_netio`'s frame encoder.
/// Server frames are NOT masked (RFC 6455 §5.3).
fn write_ws_frame(stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Binary,
        mask: None, // server → client: no mask
        payload: data.to_vec(),
    };
    stream.write_all(&frame.encode())?;
    stream.flush()
}

/// A WebSocket relay acceptor that listens for browser peer connections
/// and forwards relay frames to/from the UDP relay server.
pub struct RelayWsAcceptor {
    listener: TcpListener,
    server: Arc<Mutex<RelayServer>>,
}

impl RelayWsAcceptor {
    /// Bind a TCP listener for browser WS connections.
    pub fn bind(addr: std::net::SocketAddr, server: Arc<Mutex<RelayServer>>) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self { listener, server })
    }

    /// The bound address.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept and serve browser WS connections until `stop` is set.
    /// Each connection gets its own relay session; frames are forwarded
    /// through the relay server's UDP path. The listener fd is registered
    /// with the shared reactor for edge-triggered wake — no blind sleep.
    pub fn serve(&self, stop: &AtomicBool) -> io::Result<()> {
        use std::os::unix::io::AsRawFd;
        self.listener.set_nonblocking(true)?;

        // Register listener fd with shared reactor.
        let listener_fd = self.listener.as_raw_fd();
        let token = foundation_nativeapis::native::poll::Token(0xF300);
        let reactor = foundation_nativeapis::native::fd::Reactor::get().ok();
        if let Some(ref r) = reactor {
            r.register(listener_fd, token, foundation_nativeapis::native::poll::Interest::READABLE).ok();
        }

        while !stop.load(Ordering::Relaxed) {
            let (mut stream, _peer_addr) = match self.listener.accept() {
                Ok(c) => c,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if let Some(ref r) = reactor {
                        r.clear(token, foundation_nativeapis::native::fd::Ready::READABLE);
                        let deadline = Instant::now() + Duration::from_millis(50);
                        while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
                            if r.is_ready(token) {
                                r.clear(token, foundation_nativeapis::native::fd::Ready::READABLE);
                                break;
                            }
                            std::hint::spin_loop();
                            thread::sleep(Duration::from_millis(1));
                        }
                    } else {
                        thread::sleep(Duration::from_millis(50));
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };

            // Perform WebSocket upgrade.
            let ws_key = match parse_ws_key(&mut stream) {
                Ok(k) => k,
                Err(_) => continue,
            };
            let accept = compute_accept_key(&ws_key);
            if send_ws_upgrade(&mut stream, &accept).is_err() {
                continue;
            }

            // Hand off to a thread that reads relay frames, forwards
            // through the relay server, and writes responses back.
            let server = Arc::clone(&self.server);
            thread::spawn(move || {
                let peer_id = serve_ws_session(&mut stream, &server);
                if let Some(pid) = peer_id {
                    let mut srv = server.lock().expect("relay lock");
                    srv.detach(&pid);
                }
            });
        }
        Ok(())
    }
}

/// Serve one browser WS session: read relay frames, forward through the
/// relay server, write relay responses back as WS frames.
fn serve_ws_session(
    stream: &mut TcpStream,
    server: &Arc<Mutex<RelayServer>>,
) -> Option<PeerId> {
    // The first frame from the browser carries the relay payload.
    let first_frame = read_ws_frame(stream).ok()??;
    let relay_frame = RelayFrame::decode(&first_frame)?;
    let peer_id = relay_frame.dst_peer_id;

    {
        let mut srv = server.lock().expect("relay lock");
        if !srv.attach(peer_id) {
            return None;
        }
    }

    // Main loop: read relay frames → forward → write responses.
    loop {
        match read_ws_frame(stream) {
            Ok(Some(data)) => {
                let mut srv = server.lock().expect("relay lock");
                if let Some(fwd) = srv.forward(peer_id, &data) {
                    let resp = RelayFrame::new(fwd.dst, fwd.ciphertext);
                    if write_ws_frame(stream, &resp.encode()).is_err() {
                        return Some(peer_id);
                    }
                }
            }
            Ok(None) => return Some(peer_id), // close frame
            Err(_) => return Some(peer_id),
        }
    }
}
