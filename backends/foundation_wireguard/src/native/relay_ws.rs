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
//! HOW: Each WS connection is a relay session. The browser identifies
//! itself by the `Sec-WebSocket-Protocol` header (carrying its PeerId
//! in base64). Frames use the binary opcode and carry the same
//! `RelayFrame` format as the UDP relay path.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

use crate::shared::membership::PeerId;
use crate::shared::relay::RelayFrame;
use super::relay::RelayServer;

/// SHA-1 hash of the WebSocket key + magic GUID, base64-encoded.
fn compute_ws_accept_key(key: &str) -> String {
    /// Minimal SHA-1 for WebSocket handshake (RFC 6455 §4.2.2).
    /// Only needs to compute one hash — full sha1 crate is overkill.
    fn sha1(data: &[u8]) -> [u8; 20] {
        // RFC 3174 SHA-1 implementation.
        let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
        let ml = data.len() as u64 * 8;
        let mut padded = data.to_vec();
        padded.push(0x80);
        while (padded.len() % 64) != 56 { padded.push(0); }
        padded.extend_from_slice(&ml.to_be_bytes());

        for chunk in padded.chunks(64) {
            let mut w = [0u32; 80];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([chunk[i*4], chunk[i*4+1], chunk[i*4+2], chunk[i*4+3]]);
            }
            for i in 16..80 {
                w[i] = (w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16]).rotate_left(1);
            }
            let [mut a, mut b, mut c, mut d, mut e] = h;
            for i in 0..80 {
                let (f, k) = match i {
                    0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                    20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                    _ => (b ^ c ^ d, 0xCA62C1D6),
                };
                let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[i]);
                e = d; d = c; c = b.rotate_left(30); b = a; a = temp;
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
        }
        let mut out = [0u8; 20];
        for i in 0..5 { out[i*4..(i+1)*4].copy_from_slice(&h[i].to_be_bytes()); }
        out
    }
    let hash = sha1(&[key.as_bytes(), b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"].concat());
    B64.encode(&hash)
}

/// Read the HTTP upgrade request from a TcpStream and extract the WebSocket key.
fn parse_ws_upgrade(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut ws_key = String::new();
    for line in req.lines() {
        if let Some(val) = line.strip_prefix("Sec-WebSocket-Key:") {
            ws_key = val.trim().to_string();
        }
    }
    if ws_key.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "no Sec-WebSocket-Key"));
    }
    Ok(ws_key)
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

/// Read one WebSocket frame (RFC 6455 base framing).
/// Returns `Ok(None)` on connection close.
fn read_ws_frame(stream: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 2];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let mut len = (header[1] & 0x7F) as u64;

    if len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext)?;
        len = u16::from_be_bytes(ext) as u64;
    } else if len == 127 {
        let mut ext = [0u8; 8];
        stream.read_exact(&mut ext)?;
        len = u64::from_be_bytes(ext);
    }

    let mut mask_key = [0u8; 4];
    if masked {
        stream.read_exact(&mut mask_key)?;
    }

    if opcode == 0x8 { return Ok(None); } // Close frame

    let mut payload = vec![0u8; len as usize];
    if len > 0 {
        stream.read_exact(&mut payload)?;
    }
    if masked {
        for i in 0..payload.len() {
            payload[i] ^= mask_key[i % 4];
        }
    }
    Ok(Some(payload))
}

/// Write a WebSocket binary frame.
fn write_ws_frame(stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(10 + data.len());
    frame.push(0x82); // FIN + binary opcode
    if data.len() < 126 {
        frame.push(data.len() as u8);
    } else if data.len() <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(data.len() as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(data.len() as u64).to_be_bytes());
    }
    frame.extend_from_slice(data);
    stream.write_all(&frame)?;
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
    /// through the relay server's UDP path.
    pub fn serve(&self, stop: &AtomicBool) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;
        while !stop.load(Ordering::Relaxed) {
            let (mut stream, peer_addr) = match self.listener.accept() {
                Ok(c) => c,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(e),
            };

            // Perform WebSocket upgrade.
            let ws_key = match parse_ws_upgrade(&mut stream) {
                Ok(k) => k,
                Err(_) => continue,
            };
            let accept = compute_ws_accept_key(&ws_key);
            if send_ws_upgrade(&mut stream, &accept).is_err() {
                continue;
            }

            // Register a relay session for this browser peer.
            // The peer identity arrives in the first relay frame.
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

/// Serve one browser WS session: read relay frames, forward through the relay server,
/// write relay responses back as WS frames.
fn serve_ws_session(
    stream: &mut TcpStream,
    server: &Arc<Mutex<RelayServer>>,
) -> Option<PeerId> {
    // The browser always sends the first frame — a join/attach frame
    // carrying its PeerId. Register with the relay server.
    let first_frame = read_ws_frame(stream).ok()??;
    let relay_frame = RelayFrame::decode(&first_frame)?;
    let peer_id = relay_frame.dst_peer_id; // browser uses dst as its own id for attach

    {
        let mut srv = server.lock().expect("relay lock");
        if !srv.attach(peer_id) {
            return None;
        }
    }

    // Main loop: read frames from browser, forward to relay, write responses back.
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
