//! WebSocket test server for integration testing.
//!
//! WHY: Provides a real WebSocket server for integration tests without external dependencies.
//! Built on stdlib TCP with hand-crafted WebSocket frame handling.
//!
//! WHAT: `WebSocketEchoServer` that handles WebSocket upgrade and echoes messages back.
//!
//! HOW: Uses stdlib's `TcpListener`, performs HTTP upgrade handshake, then echoes
//! WebSocket frames back to the client.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use base64::Engine;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{
    http_streams, IncomingRequestParts, Proto, SimpleHeaders, SimpleMethod,
};
use sha1::{Digest, Sha1};

/// The magic GUID for Sec-WebSocket-Accept computation.
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Compute Sec-WebSocket-Accept from client key.
fn compute_accept_key(client_key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// WebSocket opcodes.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x0 => Some(Opcode::Continuation),
            0x1 => Some(Opcode::Text),
            0x2 => Some(Opcode::Binary),
            0x8 => Some(Opcode::Close),
            0x9 => Some(Opcode::Ping),
            0xA => Some(Opcode::Pong),
            _ => None,
        }
    }
}

/// Decode a WebSocket frame from the stream.
fn decode_frame(stream: &mut TcpStream) -> Option<(bool, Opcode, Option<[u8; 4]>, Vec<u8>)> {
    // Read first 2 bytes
    let mut header = [0u8; 2];
    tracing::info!("WebSocketEchoServer: decode_frame - attempting to read header");
    if stream.read_exact(&mut header).is_err() {
        tracing::info!("WebSocketEchoServer: decode_frame - failed to read header");
        return None;
    }
    tracing::info!(
        "WebSocketEchoServer: decode_frame - read header: [{:#04x}, {:#04x}]",
        header[0],
        header[1]
    );

    let fin = (header[0] & 0x80) != 0;
    let opcode = Opcode::from_byte(header[0] & 0x0F)?;
    let masked = (header[1] & 0x80) != 0;
    let length_byte = header[1] & 0x7F;

    tracing::info!(
        "WebSocketEchoServer: decode_frame - fin={}, opcode={:?}, masked={}, len={}",
        fin,
        opcode,
        masked,
        length_byte
    );

    // Determine payload length
    let payload_len: usize = match length_byte {
        126 => {
            let mut buf = [0u8; 2];
            if stream.read_exact(&mut buf).is_err() {
                return None;
            }
            u16::from_be_bytes(buf) as usize
        }
        127 => {
            let mut buf = [0u8; 8];
            if stream.read_exact(&mut buf).is_err() {
                return None;
            }
            u64::from_be_bytes(buf) as usize
        }
        n => n as usize,
    };

    // Read masking key if present
    let mask = if masked {
        let mut key = [0u8; 4];
        if stream.read_exact(&mut key).is_err() {
            return None;
        }
        Some(key)
    } else {
        None
    };

    // Read payload
    let mut payload = vec![0u8; payload_len];
    if stream.read_exact(&mut payload).is_err() {
        return None;
    }

    // Unmask payload if masked
    if let Some(mask_key) = mask {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask_key[i % 4];
        }
    }

    Some((fin, opcode, mask, payload))
}

/// Encode a WebSocket frame to bytes.
fn encode_frame(fin: bool, opcode: Opcode, mask: Option<[u8; 4]>, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();

    // First byte: FIN + opcode
    let first_byte = if fin { 0x80 } else { 0x00 } | (opcode as u8);
    buf.push(first_byte);

    // Second byte: MASK bit + payload length
    let mask_bit: u8 = if mask.is_some() { 0x80 } else { 0x00 };
    let len = payload.len();

    if len < 126 {
        buf.push(mask_bit | (len as u8));
    } else if len <= 65535 {
        buf.push(mask_bit | 126);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        buf.push(mask_bit | 127);
        buf.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Masking key + payload
    if let Some(mask_key) = mask {
        buf.extend_from_slice(&mask_key);
        let mut masked_payload = payload.to_vec();
        for (i, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= mask_key[i % 4];
        }
        buf.extend_from_slice(&masked_payload);
    } else {
        buf.extend_from_slice(payload);
    }

    buf
}

/// Test WebSocket server for integration testing.
///
/// # Purpose (WHY)
///
/// Provides a real WebSocket server for testing WebSocket clients without external dependencies.
/// Uses stdlib TCP with manually crafted WebSocket frames.
///
/// # What it does
///
/// Starts a local WebSocket server on a random port, performs HTTP upgrade handshake,
/// and echoes messages back to the client. Runs in background thread.
///
/// # Examples
///
/// ```rust
/// use foundation_testing::http::WebSocketEchoServer;
///
/// let server = WebSocketEchoServer::start();
///
/// // Use server.ws_url("/") in WebSocket client tests
/// // let mut client = WebSocketClient::connect(server.ws_url("/")).unwrap();
/// // client.send(WebSocketMessage::Text("hello".to_string())).unwrap();
/// // let response = client.recv().unwrap();
/// ```
pub struct WebSocketEchoServer {
    addr: String,
    _handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl WebSocketEchoServer {
    /// Start a new WebSocket echo server on random port.
    ///
    /// # Returns
    ///
    /// A running `WebSocketEchoServer` that will echo messages back.
    #[must_use]
    pub fn start() -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("Failed to bind test WebSocket server");
        let addr = format!("ws://{}", listener.local_addr().unwrap());

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let handler_clone = Arc::clone(&running_clone);
                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection(stream, &handler_clone) {
                                tracing::info!("WebSocketEchoServer connection error: {e}");
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::info!("WebSocketEchoServer accept error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            _handle: Some(handle),
            running,
        }
    }

    /// Start a new WebSocket echo server with custom subprotocol support.
    ///
    /// # Arguments
    ///
    /// * `supported_protocols` - Comma-separated list of supported subprotocols
    ///
    /// # Returns
    ///
    /// A running `WebSocketEchoServer` that echoes back the selected subprotocol.
    #[must_use]
    pub fn with_subprotocols(supported_protocols: &str) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("Failed to bind test WebSocket server");
        let addr = format!(
            "ws://{}|subprotocols={}",
            listener.local_addr().unwrap(),
            supported_protocols
        );

        let supported = supported_protocols.to_string();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("Failed to set non-blocking");

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let handler_clone = Arc::clone(&running_clone);
                        let supported_clone = supported.clone();
                        thread::spawn(move || {
                            if let Err(e) = Self::handle_connection_with_protocols(
                                stream,
                                &supported_clone,
                                &handler_clone,
                            ) {
                                tracing::info!("WebSocketEchoServer connection error: {e}");
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::info!("WebSocketEchoServer accept error: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            _handle: Some(handle),
            running,
        }
    }

    /// Get full WebSocket URL for a path on this test server.
    #[must_use]
    pub fn ws_url(&self, path: &str) -> String {
        format!("{}{}", self.addr, path)
    }

    /// Get base URL of this test server.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.addr
    }

    /// Handle a WebSocket connection with subprotocol support.
    fn handle_connection_with_protocols(
        mut stream: TcpStream,
        supported_protocols: &str,
        running: &AtomicBool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Parse HTTP request for upgrade
        let (_method, _url, _proto, headers) = Self::read_http_request(&mut stream)?;

        // Check for WebSocket upgrade
        let upgrade_header = headers
            .get(&foundation_core::wire::simple_http::SimpleHeader::UPGRADE)
            .and_then(|v| v.first())
            .map(|s| s.to_lowercase());

        if upgrade_header.as_deref() != Some("websocket") {
            // Not a WebSocket upgrade request, send 400
            let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
            stream.write_all(response.as_bytes())?;
            return Ok(());
        }

        // Get Sec-WebSocket-Key
        let ws_key = headers
            .get(&foundation_core::wire::simple_http::SimpleHeader::SEC_WEBSOCKET_KEY)
            .and_then(|v| v.first())
            .cloned()
            .ok_or("Missing Sec-WebSocket-Key")?;

        // Check requested protocols
        let selected_protocol = headers
            .get(&foundation_core::wire::simple_http::SimpleHeader::SEC_WEBSOCKET_PROTOCOL)
            .and_then(|v| v.first())
            .and_then(|requested| {
                // Find first matching protocol
                requested.split(',').find_map(|req| {
                    let req = req.trim();
                    supported_protocols
                        .split(',')
                        .find(|sup| sup.trim() == req)
                        .map(|s| s.to_string())
                })
            });

        // Compute accept key
        let accept_key = compute_accept_key(&ws_key);

        // Build upgrade response
        let mut response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n",
            accept_key
        );

        if let Some(protocol) = selected_protocol {
            response.push_str(&format!("Sec-WebSocket-Protocol: {}\r\n", protocol));
        }

        response.push_str("\r\n");
        stream.write_all(response.as_bytes())?;
        stream.flush()?;

        // Now handle WebSocket frames
        Self::handle_websocket_frames(&mut stream, running)
    }

    /// Handle a WebSocket connection.
    fn handle_connection(
        mut stream: TcpStream,
        running: &AtomicBool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Parse HTTP request for upgrade
        let (_method, _url, _proto, headers) = Self::read_http_request(&mut stream)?;

        // Check for WebSocket upgrade
        let upgrade_header = headers
            .get(&foundation_core::wire::simple_http::SimpleHeader::UPGRADE)
            .and_then(|v| v.first())
            .map(|s| s.to_lowercase());

        if upgrade_header.as_deref() != Some("websocket") {
            // Not a WebSocket upgrade request, send 400
            let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
            stream.write_all(response.as_bytes())?;
            return Ok(());
        }

        // Get Sec-WebSocket-Key
        let ws_key = headers
            .get(&foundation_core::wire::simple_http::SimpleHeader::SEC_WEBSOCKET_KEY)
            .and_then(|v| v.first())
            .cloned()
            .ok_or("Missing Sec-WebSocket-Key")?;

        // Compute accept key
        let accept_key = compute_accept_key(&ws_key);

        // Build upgrade response
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\
             \r\n",
            accept_key
        );
        stream.write_all(response.as_bytes())?;
        stream.flush()?;

        // Now handle WebSocket frames
        Self::handle_websocket_frames(&mut stream, running)
    }

    /// Read HTTP request from stream (only headers needed for WebSocket handshake).
    fn read_http_request(
        stream: &mut TcpStream,
    ) -> Result<
        (
            SimpleMethod,
            foundation_core::wire::simple_http::SimpleUrl,
            Proto,
            SimpleHeaders,
        ),
        Box<dyn std::error::Error>,
    > {
        let conn = RawStream::from_tcp(stream.try_clone()?)?;
        let request_streams = http_streams::send::http_streams(conn);
        let request_reader = request_streams.next_request();

        let parts: Vec<IncomingRequestParts> = request_reader
            .into_iter()
            .filter_map(|item| item.ok())
            .filter(|item| !matches!(item, IncomingRequestParts::SKIP))
            .collect();

        if parts.len() < 2 {
            return Err("Invalid HTTP request".into());
        }

        let headers_part = &parts[1];
        let intros_part = &parts[0];

        let IncomingRequestParts::Intro(method, url, proto) = intros_part else {
            return Err("Invalid HTTP request intro".into());
        };

        let IncomingRequestParts::Headers(headers) = headers_part else {
            return Err("Invalid HTTP request headers".into());
        };

        Ok((method.clone(), url.clone(), proto.clone(), headers.clone()))
    }

    /// Handle WebSocket frames - echo back messages, respond to pings.
    ///
    /// The server never proactively closes the connection. It only exits when:
    /// 1. Client sends a Close frame (graceful shutdown)
    /// 2. Client disconnects (read error/EOF)
    /// 3. The `running` flag is set to false (test harness shutdown)
    fn handle_websocket_frames(
        stream: &mut TcpStream,
        running: &AtomicBool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // No read timeout - server should wait indefinitely for client messages
        // during tests. The running flag is only checked after a complete frame
        // is received, so it can be used for graceful shutdown without affecting
        // connection lifetime.
        stream.set_read_timeout(None)?;

        while running.load(Ordering::Relaxed) {
            tracing::info!("WebsocketEchoServer: next loop for message");
            match decode_frame(stream) {
                Some((fin, opcode, _mask, payload)) => {
                    tracing::info!(
                        "WebSocketEchoServer: received opcode={:?}, payload_len={}",
                        opcode,
                        payload.len()
                    );
                    match opcode {
                        Opcode::Text | Opcode::Binary => {
                            tracing::info!("WebSocketEchoServer: received text|binary");
                            // Echo back the message
                            let response = encode_frame(fin, opcode, None, &payload);
                            tracing::info!("WebSocketEchoServer: echoing {} bytes", response.len());
                            stream.write_all(&response)?;
                            stream.flush()?;
                            tracing::info!("WebSocketEchoServer: echo sent");
                        }
                        Opcode::Ping => {
                            tracing::info!("WebSocketEchoServer: received ping");
                            // Respond with Pong (same payload)
                            let response = encode_frame(true, Opcode::Pong, None, &payload);
                            stream.write_all(&response)?;
                            stream.flush()?;
                        }
                        Opcode::Close => {
                            tracing::info!("WebSocketEchoServer: closing connection");
                            // Echo close frame back and exit
                            let response = encode_frame(true, Opcode::Close, None, &payload);
                            stream.write_all(&response)?;
                            stream.flush()?;
                            break;
                        }
                        Opcode::Pong | Opcode::Continuation => {
                            tracing::info!("WebSocketEchoServer: received pong|continuation");
                            // Ignore
                        }
                    }
                }
                None => {
                    tracing::info!("WebSocketEchoServer: no message received, breaking");
                    // Client disconnected or error reading - exit cleanly
                    break;
                }
            }
        }

        tracing::info!("WebSocketEchoServer: stopping server");
        Ok(())
    }
}

impl Drop for WebSocketEchoServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_start() {
        let server = WebSocketEchoServer::start();
        assert!(server.base_url().starts_with("ws://127.0.0.1:"));
    }

    #[test]
    fn test_compute_accept_key_rfc_vector() {
        let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
        let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
        let result = compute_accept_key(client_key);
        assert_eq!(result, expected);
    }
}
