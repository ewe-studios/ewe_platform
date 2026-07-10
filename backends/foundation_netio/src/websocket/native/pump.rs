//! WebSocket byte-pump — bridges WS messages ⇄ byte pipes (F39).
//!
//! WHY: The WS transport needs a non-generic valtron task that opens a TCP
//! connection, performs the WS upgrade handshake, and then enters a byte-pipe
//! ↔ WS frame loop. This avoids the generic `WebSocketClient<R>` which can't
//! be boxed into `Transport::open()`.
//!
//! WHAT: [`WsBytePump`] connects via TCP, sends the HTTP upgrade request,
//! validates the 101 response, then enters a read/write loop. One I/O operation
//! per `next_status()` call — never loops inside a phase.
//!
//! Pattern: flat state machine matching [`ReconnectingWebSocketTask`].
//! Each poll does exactly one read or one write, then returns.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use foundation_core::valtron::{PipeReceiver, PipeSender, TaskIterator, TaskStatus};

use crate::websocket::shared::frame::{generate_mask, Opcode, WebSocketFrame};
use crate::websocket::shared::handshake::{build_upgrade_request, compute_accept_key};
use crate::websocket::shared::error::WebSocketError;
use crate::simple_http::shared::{Http11, RenderHttp, SimpleHeaders, Status};

#[derive(Debug, PartialEq, Eq)]
enum Phase {
    Connecting,
    HandshakeSent,
    ReadingHandshake,
    Open,
    Done,
}

pub struct WsBytePump {
    phase: Phase,
    host: String,
    port: u16,
    path: String,
    extra_headers: SimpleHeaders,
    send_rx: PipeReceiver<Bytes>,
    head_tx: PipeSender<(Status, SimpleHeaders)>,
    body_tx: PipeSender<Bytes>,
    trailer_tx: PipeSender<SimpleHeaders>,
    stream: Option<TcpStream>,
    ws_key: String,
    response_buf: Vec<u8>,
    frame_buf: BytesMut,
    /// How many outbound chunks we've sent this poll. Capped to avoid hogging.
    outbound_sent: usize,
}

impl WsBytePump {
    #[must_use]
    pub fn new(
        host: String,
        port: u16,
        path: String,
        extra_headers: SimpleHeaders,
        send_rx: PipeReceiver<Bytes>,
        head_tx: PipeSender<(Status, SimpleHeaders)>,
        body_tx: PipeSender<Bytes>,
        trailer_tx: PipeSender<SimpleHeaders>,
    ) -> Self {
        Self {
            phase: Phase::Connecting,
            host, port, path, extra_headers,
            send_rx, head_tx, body_tx, trailer_tx,
            stream: None,
            ws_key: String::new(),
            response_buf: Vec::with_capacity(4096),
            frame_buf: BytesMut::with_capacity(8192),
            outbound_sent: 0,
        }
    }

    fn fail(&mut self, status: Status) -> Option<TaskStatus<(), (), foundation_core::valtron::NoSpawner>> {
        let _ = self.head_tx.try_send((status, SimpleHeaders::new()));
        self.head_tx.close();
        self.body_tx.close();
        self.trailer_tx.close();
        self.phase = Phase::Done;
        None
    }

    fn write_frame(&mut self, frame: &WebSocketFrame) -> bool {
        match self.stream.as_mut() {
            Some(stream) => {
                let encoded = frame.encode();
                match stream.write_all(&encoded) {
                    Ok(()) => { stream.flush().ok(); true }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
                    Err(_) => false,
                }
            }
            None => false,
        }
    }

    fn extract_header(&self, buf: &[u8], name: &str) -> Option<String> {
        let head = std::str::from_utf8(buf).ok()?;
        let lower = name.to_ascii_lowercase();
        for line in head.lines().skip(1) {
            if let Some((k, v)) = line.split_once(": ") {
                if k.to_ascii_lowercase() == lower {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    fn response_complete(buf: &[u8]) -> bool {
        buf.windows(4).any(|w| w == b"\r\n\r\n")
    }

    fn http_status_code(buf: &[u8]) -> Option<u16> {
        let head = std::str::from_utf8(buf).ok()?;
        let status_line = head.lines().next()?;
        let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
        parts.get(1)?.parse().ok()
    }
}

impl TaskIterator for WsBytePump {
    type Ready = ();
    type Pending = ();
    type Spawner = foundation_core::valtron::NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), Self::Spawner>> {
        match self.phase {
            Phase::Connecting => {
                let addr = format!("{}:{}", self.host, self.port);
                let sockaddr = match addr.parse() {
                    Ok(a) => a,
                    Err(_) => return self.fail(Status::BadGateway),
                };
                match TcpStream::connect_timeout(&sockaddr, Duration::from_secs(10)) {
                    Ok(stream) => {
                        stream.set_nonblocking(true).ok();
                        self.stream = Some(stream);
                        self.phase = Phase::HandshakeSent;
                        Some(TaskStatus::Pending(()))
                    }
                    Err(_) => self.fail(Status::BadGateway),
                }
            }
            Phase::HandshakeSent => {
                self.ws_key = crate::websocket::shared::handshake::generate_websocket_key();
                let mut request = match build_upgrade_request(
                    &self.host, &self.path, &self.ws_key, None,
                ) {
                    Ok(r) => r,
                    Err(_) => return self.fail(Status::BadRequest),
                };
                for (name, values) in &self.extra_headers {
                    for v in values {
                        request.headers.entry(name.clone()).or_default().push(v.clone());
                    }
                }
                let rendered = match Http11::request(request).http_render_string() {
                    Ok(r) => r,
                    Err(_) => return self.fail(Status::InternalServerError),
                };
                let raw = rendered.into_bytes();
                let stream = self.stream.as_mut().unwrap();
                match stream.write_all(&raw) {
                    Ok(()) => {
                        stream.flush().ok();
                        self.phase = Phase::ReadingHandshake;
                        Some(TaskStatus::Pending(()))
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        Some(TaskStatus::Delayed(Duration::from_millis(1)))
                    }
                    Err(_) => self.fail(Status::BadGateway),
                }
            }
            Phase::ReadingHandshake => {
                let mut buf = [0u8; 4096];
                let stream = self.stream.as_mut().unwrap();
                match stream.read(&mut buf) {
                    Ok(0) => self.fail(Status::BadGateway),
                    Ok(n) => {
                        self.response_buf.extend_from_slice(&buf[..n]);
                        if !Self::response_complete(&self.response_buf) {
                            return Some(TaskStatus::Pending(()));
                        }
                        let header_end = self.response_buf
                            .windows(4).position(|w| w == b"\r\n\r\n")
                            .unwrap() + 4;
                        let code = Self::http_status_code(&self.response_buf[..header_end])
                            .unwrap_or(500);
                        if code == 101 {
                            let got_accept = self.extract_header(
                                &self.response_buf[..header_end], "sec-websocket-accept",
                            ).unwrap_or_default();
                            let expected = compute_accept_key(&self.ws_key);
                            if got_accept != expected {
                                return self.fail(Status::BadGateway);
                            }
                            let _ = self.head_tx.try_send((Status::SwitchingProtocols, SimpleHeaders::new()));
                            self.head_tx.close();
                            self.phase = Phase::Open;
                            Some(TaskStatus::Pending(()))
                        } else {
                            self.fail(match code {
                                400 => Status::BadRequest,
                                404 => Status::NotFound,
                                500..=599 => Status::InternalServerError,
                                _ => Status::BadGateway,
                            })
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        Some(TaskStatus::Delayed(Duration::from_millis(1)))
                    }
                    Err(_) => self.fail(Status::BadGateway),
                }
            }
            Phase::Open => {
                const MAX_OUTBOUND: usize = 16;

                // ── Outbound: one chunk per poll (bounded) ──
                if self.outbound_sent < MAX_OUTBOUND {
                    match self.send_rx.try_recv() {
                        Ok(bytes) => {
                            self.outbound_sent += 1;
                            let frame = WebSocketFrame {
                                fin: true, opcode: Opcode::Binary,
                                mask: Some(generate_mask()), payload: bytes.to_vec(),
                            };
                            if !self.write_frame(&frame) {
                                self.body_tx.close();
                                self.trailer_tx.close();
                                self.phase = Phase::Done;
                                return Some(TaskStatus::Pending(()));
                            }
                            return Some(TaskStatus::Pending(()));
                        }
                        Err(foundation_core::valtron::TryRecvError::Closed) => {
                            let close = WebSocketFrame {
                                fin: true, opcode: Opcode::Close,
                                mask: Some(generate_mask()), payload: Vec::new(),
                            };
                            self.write_frame(&close);
                            self.body_tx.close();
                            self.trailer_tx.close();
                            self.phase = Phase::Done;
                            return Some(TaskStatus::Pending(()));
                        }
                        Err(foundation_core::valtron::TryRecvError::Empty) => {}
                    }
                }
                self.outbound_sent = 0;

                // ── Inbound: one frame per poll ──
                let stream = self.stream.as_mut().unwrap();
                match WebSocketFrame::decode_with_buffer(stream, &mut self.frame_buf) {
                    Ok(frame) => {
                        match frame.opcode {
                            Opcode::Binary | Opcode::Text => {
                                let _ = self.body_tx.try_send(Bytes::from(frame.payload));
                            }
                            Opcode::Close => {
                                let resp = WebSocketFrame {
                                    fin: true, opcode: Opcode::Close,
                                    mask: Some(generate_mask()), payload: frame.payload,
                                };
                                self.write_frame(&resp);
                                self.body_tx.close();
                                self.trailer_tx.close();
                                self.phase = Phase::Done;
                            }
                            Opcode::Ping => {
                                let pong = WebSocketFrame {
                                    fin: true, opcode: Opcode::Pong,
                                    mask: Some(generate_mask()), payload: frame.payload,
                                };
                                self.write_frame(&pong);
                            }
                            _ => {}
                        }
                        Some(TaskStatus::Pending(()))
                    }
                    Err(WebSocketError::IoError(ref e))
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        Some(TaskStatus::Delayed(Duration::from_millis(1)))
                    }
                    Err(_) => {
                        self.body_tx.close();
                        self.trailer_tx.close();
                        self.phase = Phase::Done;
                        Some(TaskStatus::Pending(()))
                    }
                }
            }
            Phase::Done => {
                self.head_tx.close();
                self.body_tx.close();
                self.trailer_tx.close();
                None
            }
        }
    }
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_core::valtron::Pipe;

    fn accept_key_for(key: &str) -> Vec<u8> {
        let accept = compute_accept_key(key);
        format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept}\r\n\
             \r\n"
        ).into_bytes()
    }

    #[test]
    fn parse_101_response() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let response = accept_key_for(key);

        let mut pump = WsBytePump::new(
            "x".into(), 0, "/".into(), SimpleHeaders::new(),
            Pipe::<Bytes>::with_depth(1).1,
            Pipe::<(Status, SimpleHeaders)>::with_depth(1).0,
            Pipe::<Bytes>::with_depth(1).0,
            Pipe::<SimpleHeaders>::with_depth(1).0,
        );
        pump.response_buf = response.clone();
        pump.response_buf.truncate(pump.response_buf.len()); // fill_from not needed
        // Re-inject into the buffer for the test.
        pump.response_buf = response;
        let header_end = pump.response_buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(WsBytePump::http_status_code(&pump.response_buf[..header_end]), Some(101));
        let got_accept = pump.extract_header(&pump.response_buf[..header_end], "sec-websocket-accept").unwrap();
        assert_eq!(got_accept, compute_accept_key(key));
    }

    #[test]
    fn parse_404_response() {
        let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec();
        let mut pump = WsBytePump::new(
            "x".into(), 0, "/".into(), SimpleHeaders::new(),
            Pipe::<Bytes>::with_depth(1).1,
            Pipe::<(Status, SimpleHeaders)>::with_depth(1).0,
            Pipe::<Bytes>::with_depth(1).0,
            Pipe::<SimpleHeaders>::with_depth(1).0,
        );
        pump.response_buf = response;
        let header_end = pump.response_buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(WsBytePump::http_status_code(&pump.response_buf[..header_end]), Some(404));
    }
}
