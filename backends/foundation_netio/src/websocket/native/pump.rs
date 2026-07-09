//! WebSocket byte-pump — bridges WS messages ⇄ byte pipes (F39).
//!
//! WHY: The WS transport needs a non-generic valtron task that opens a TCP
//! connection, performs the WS upgrade handshake, and then enters a byte-pipe
//! ↔ WS frame loop. This avoids the generic `WebSocketClient<R>` which can't
//! be boxed into `Transport::open()`.
//!
//! WHAT: [`WsBytePump`] connects via TCP, sends the HTTP upgrade request,
//! validates the 101 response, then enters a read/write loop. Outbound
//! `send_rx` bytes → WS Binary frames → TCP. Inbound TCP bytes → WS frames
//! → `body_tx`.

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

/// Valtron task that bridges WS ↔ byte pipes.
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
    response_read: usize,
    frame_buf: BytesMut,
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
            host,
            port,
            path,
            extra_headers,
            send_rx,
            head_tx,
            body_tx,
            trailer_tx,
            stream: None,
            ws_key: String::new(),
            response_buf: Vec::with_capacity(4096),
            response_read: 0,
            frame_buf: BytesMut::with_capacity(8192),
        }
    }

    fn fail(&mut self, status: Status) {
        let _ = self.head_tx.try_send((status, SimpleHeaders::new()));
        self.head_tx.close();
        self.body_tx.close();
        self.trailer_tx.close();
        self.phase = Phase::Done;
    }

    fn http_response_end(&self) -> Option<usize> {
        let view = &self.response_buf[..self.response_read];
        view.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
    }

    fn http_status_code(&self, header_end: usize) -> Option<u16> {
        let head = std::str::from_utf8(&self.response_buf[..header_end]).ok()?;
        let status_line = head.lines().next()?;
        let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
        parts.get(1)?.parse().ok()
    }

    fn http_header(&self, header_end: usize, name: &str) -> Option<String> {
        let head = std::str::from_utf8(&self.response_buf[..header_end]).ok()?;
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

    fn send_ws_frame(&mut self, frame: &WebSocketFrame) -> bool {
        if let Some(ref mut stream) = self.stream {
            let encoded = frame.encode();
            match stream.write_all(&encoded) {
                Ok(()) => { stream.flush().ok(); true }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
                Err(_) => false,
            }
        } else {
            false
        }
    }
}

impl TaskIterator for WsBytePump {
    type Ready = ();
    type Pending = ();
    type Spawner = foundation_core::valtron::NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), Self::Spawner>> {
        loop {
            match self.phase {
                Phase::Connecting => {
                    let addr = format!("{}:{}", self.host, self.port);
                    let sockaddr = addr.parse().ok()?;
                    match TcpStream::connect_timeout(&sockaddr, Duration::from_secs(10)) {
                        Ok(stream) => {
                            stream.set_nonblocking(true).ok();
                            self.stream = Some(stream);
                            self.phase = Phase::HandshakeSent;
                        }
                        Err(_) => {
                            self.fail(Status::BadGateway);
                            return None;
                        }
                    }
                }
                Phase::HandshakeSent => {
                    self.ws_key = crate::websocket::shared::handshake::generate_websocket_key();
                    let mut request = match build_upgrade_request(
                        &self.host,
                        &self.path,
                        &self.ws_key,
                        None,
                    ) {
                        Ok(r) => r,
                        Err(_) => {
                            self.fail(Status::BadRequest);
                            return None;
                        }
                    };
                    // Forward extra headers onto the upgrade request.
                    for (name, values) in self.extra_headers.iter() {
                        for v in values {
                            request.headers.entry(name.clone()).or_default().push(v.clone());
                        }
                    }
                    let rendered = match Http11::request(request).http_render_string() {
                        Ok(r) => r,
                        Err(_) => {
                            self.fail(Status::InternalServerError);
                            return None;
                        }
                    };
                    let raw = rendered.into_bytes();
                    let stream = self.stream.as_mut().unwrap();
                    match stream.write_all(&raw) {
                        Ok(()) => {
                            stream.flush().ok();
                            self.phase = Phase::ReadingHandshake;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            return Some(TaskStatus::Delayed(Duration::from_millis(1)));
                        }
                        Err(_) => {
                            self.fail(Status::BadGateway);
                            return None;
                        }
                    }
                }
                Phase::ReadingHandshake => {
                    let mut buf = [0u8; 4096];
                    let stream = self.stream.as_mut().unwrap();
                    match stream.read(&mut buf) {
                        Ok(0) => {
                            self.fail(Status::BadGateway);
                            return None;
                        }
                        Ok(n) => {
                            self.response_buf.extend_from_slice(&buf[..n]);
                            self.response_read += n;
                            if let Some(header_end) = self.http_response_end() {
                                let code = self.http_status_code(header_end).unwrap_or(500);
                                if code == 101 {
                                    let got_accept = self
                                        .http_header(header_end, "sec-websocket-accept")
                                        .unwrap_or_default();
                                    let expected = compute_accept_key(&self.ws_key);
                                    if got_accept != expected {
                                        self.fail(Status::BadGateway);
                                        return None;
                                    }
                                    let _ = self.head_tx.try_send((
                                        Status::SwitchingProtocols,
                                        SimpleHeaders::new(),
                                    ));
                                    self.head_tx.close();
                                    self.phase = Phase::Open;
                                } else {
                                    self.fail(
                                        match code {
                                            400 => Status::BadRequest,
                                            404 => Status::NotFound,
                                            500..=599 => Status::InternalServerError,
                                            _ => Status::BadGateway,
                                        },
                                    );
                                    return None;
                                }
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            return Some(TaskStatus::Delayed(Duration::from_millis(1)));
                        }
                        Err(_) => {
                            self.fail(Status::BadGateway);
                            return None;
                        }
                    }
                }
                Phase::Open => {
                    // ── Outbound ──
                    loop {
                        match self.send_rx.try_recv() {
                            Ok(bytes) => {
                                let frame = WebSocketFrame {
                                    fin: true,
                                    opcode: Opcode::Binary,
                                    mask: Some(generate_mask()),
                                    payload: bytes.to_vec(),
                                };
                                if !self.send_ws_frame(&frame) {
                                    self.body_tx.close();
                                    self.trailer_tx.close();
                                    self.phase = Phase::Done;
                                    return Some(TaskStatus::Pending(()));
                                }
                            }
                            Err(foundation_core::valtron::TryRecvError::Closed) => {
                                let close = WebSocketFrame {
                                    fin: true,
                                    opcode: Opcode::Close,
                                    mask: Some(generate_mask()),
                                    payload: Vec::new(),
                                };
                                self.send_ws_frame(&close);
                                self.body_tx.close();
                                self.trailer_tx.close();
                                self.phase = Phase::Done;
                                return Some(TaskStatus::Pending(()));
                            }
                            Err(foundation_core::valtron::TryRecvError::Empty) => break,
                        }
                    }

                    // ── Inbound ──
                    let stream = self.stream.as_mut().unwrap();
                    match WebSocketFrame::decode_with_buffer(stream, &mut self.frame_buf) {
                        Ok(frame) => {
                            match frame.opcode {
                                Opcode::Binary | Opcode::Text => {
                                    let _ = self.body_tx.try_send(Bytes::from(frame.payload));
                                }
                                Opcode::Close => {
                                    let resp = WebSocketFrame {
                                        fin: true,
                                        opcode: Opcode::Close,
                                        mask: Some(generate_mask()),
                                        payload: frame.payload,
                                    };
                                    self.send_ws_frame(&resp);
                                    self.body_tx.close();
                                    self.trailer_tx.close();
                                    self.phase = Phase::Done;
                                    return Some(TaskStatus::Pending(()));
                                }
                                Opcode::Ping => {
                                    let pong = WebSocketFrame {
                                        fin: true,
                                        opcode: Opcode::Pong,
                                        mask: Some(generate_mask()),
                                        payload: frame.payload,
                                    };
                                    self.send_ws_frame(&pong);
                                }
                                _ => {}
                            }
                        }
                        Err(WebSocketError::IoError(ref e))
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            return Some(TaskStatus::Delayed(Duration::from_millis(1)));
                        }
                        Err(_) => {
                            self.body_tx.close();
                            self.trailer_tx.close();
                            self.phase = Phase::Done;
                            return Some(TaskStatus::Pending(()));
                        }
                    }
                    return Some(TaskStatus::Pending(()));
                }
                Phase::Done => {
                    self.head_tx.close();
                    self.body_tx.close();
                    self.trailer_tx.close();
                    return None;
                }
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
        )
        .into_bytes()
    }

    /// Unit test: verify the HTTP response parser extracts status code 101
    /// and the Sec-WebSocket-Accept header.
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
        pump.response_read = response.len();
        let header_end = pump.http_response_end().unwrap();
        assert_eq!(pump.http_status_code(header_end), Some(101));
        let got_accept = pump.http_header(header_end, "sec-websocket-accept").unwrap();
        assert_eq!(got_accept, compute_accept_key(key));
    }

    /// Unit test: parse a non-101 response produces the right failure.
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
        pump.response_buf = response.clone();
        pump.response_read = response.len();
        let header_end = pump.http_response_end().unwrap();
        assert_eq!(pump.http_status_code(header_end), Some(404));
    }

    // Integration test (`ws_pump_round_trip_via_fake_server`) deferred:
    // needs the `Http11::request().http_render_string()` rendering path
    // verified independently. See decision 13 and the pump module docs.
}
