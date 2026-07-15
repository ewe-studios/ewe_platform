//! WebTransport session: accept/connect, streams, datagrams, close (spec-55, F06).

use std::collections::VecDeque;

use bytes::Bytes;
use foundation_core::valtron::Stream;

use crate::quic::{QuicConnError, QuicConnection, QuicStreamError};

use super::proto::{self, CapsuleType, WtProtocolError};

// ── WtStreamError ──
#[derive(Debug)]
pub enum WtStreamError {
    Closed(WtProtocolError),
    Transport(String),
    DatagramsNotSupported,
}
impl std::fmt::Display for WtStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed(e) => write!(f, "session closed: {e}"),
            Self::Transport(m) => write!(f, "transport error: {m}"),
            Self::DatagramsNotSupported => write!(f, "datagrams not supported"),
        }
    }
}
impl std::error::Error for WtStreamError {}

// ── WtSessionState ──
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WtSessionState { Connecting, Open, Draining, Closed }

// ── WtSession<C: QuicConnection> ──
/// Generic WebTransport session over a real QUIC connection. The `C` type
/// parameter is the concrete QUIC backend.
pub struct WtSession<C: QuicConnection> {
    pub conn: C,
    pub state: WtSessionState,
    datagrams_enabled: bool,
    recv_datagrams: VecDeque<Bytes>,
    peer_close: Option<(u32, String)>,
    send_datagrams: VecDeque<Bytes>,
}

impl<C: QuicConnection> WtSession<C> {
    pub fn new(conn: C, datagrams_enabled: bool) -> Self {
        Self {
            conn,
            state: WtSessionState::Connecting,
            datagrams_enabled,
            recv_datagrams: VecDeque::new(),
            peer_close: None,
            send_datagrams: VecDeque::new(),
        }
    }

    pub fn on_connected(&mut self) { self.state = WtSessionState::Open; }

    pub fn on_capsule(&mut self, capsule: CapsuleType) -> bool {
        match capsule {
            CapsuleType::Datagram(data) => {
                if self.datagrams_enabled { self.recv_datagrams.push_back(Bytes::from(data)); }
                false
            }
            CapsuleType::CloseSession { code, reason } => {
                self.peer_close = Some((code, reason));
                self.state = WtSessionState::Closed;
                true
            }
            CapsuleType::Drain => { self.state = WtSessionState::Draining; true }
            CapsuleType::Unknown(_, _) => false,
        }
    }

    pub fn open_bidi(&mut self) -> Stream<Result<C::BidiStream, QuicStreamError>, ()> { self.conn.open_bidi() }
    pub fn accept_bidi(&mut self) -> Stream<Result<C::BidiStream, QuicConnError>, ()> { self.conn.accept_bidi() }
    pub fn open_uni(&mut self) -> Stream<Result<C::SendStream, QuicStreamError>, ()> { self.conn.open_send() }
    pub fn accept_uni(&mut self) -> Stream<Result<C::RecvStream, QuicConnError>, ()> { self.conn.accept_recv() }

    pub fn datagrams_enabled(&self) -> bool { self.datagrams_enabled }
    pub fn is_open(&self) -> bool { self.state == WtSessionState::Open }
    pub fn is_closed(&self) -> bool { self.state == WtSessionState::Closed }
    pub fn close_info(&self) -> Option<(u32, &str)> {
        self.peer_close.as_ref().map(|(c, r)| (*c, r.as_str()))
    }

    pub fn queue_datagram(&mut self, data: Bytes) -> Result<(), WtStreamError> {
        if !self.datagrams_enabled { return Err(WtStreamError::DatagramsNotSupported); }
        if self.is_closed() {
            return Err(WtStreamError::Closed(WtProtocolError::SessionClosed { code: 0, reason: String::new() }));
        }
        self.send_datagrams.push_back(data);
        Ok(())
    }

    pub fn flush_datagrams(&mut self) -> Stream<Result<(), QuicStreamError>, ()> {
        while let Some(data) = self.send_datagrams.pop_front() {
            match self.conn.send_datagram(&data) {
                Stream::Next(Ok(())) => continue,
                Stream::Next(Err(e)) => return Stream::Next(Err(e)),
                _ => { self.send_datagrams.push_front(data); return Stream::Pending(()); }
            }
        }
        Stream::Next(Ok(()))
    }

    pub fn pump_recv_datagrams(&mut self) {
        loop {
            match self.conn.recv_datagram() {
                Stream::Next(Ok(Some(data))) => self.recv_datagrams.push_back(data),
                _ => break,
            }
        }
    }

    pub fn try_recv_datagram(&mut self) -> Stream<Result<Option<Bytes>, WtStreamError>, ()> {
        self.pump_recv_datagrams();
        if let Some((code, reason)) = &self.peer_close {
            return Stream::Next(Err(WtStreamError::Closed(WtProtocolError::SessionClosed { code: *code, reason: reason.clone() })));
        }
        if let Some(data) = self.recv_datagrams.pop_front() { Stream::Next(Ok(Some(data))) } else { Stream::Pending(()) }
    }

    pub fn build_close_capsule(code: u32, reason: &str) -> Vec<u8> { proto::encode_close_session(code, reason) }
    pub fn close(&mut self, code: u64, reason: &[u8]) { self.conn.close(code, reason); self.state = WtSessionState::Closed; }
}

// ── WtAcceptor<C: QuicConnection> ──
pub struct WtAcceptor<C: QuicConnection> {
    accepted: VecDeque<WtSession<C>>,
}

impl<C: QuicConnection> WtAcceptor<C> {
    pub fn new() -> Self { Self { accepted: VecDeque::new() } }
    pub fn queue_session(&mut self, session: WtSession<C>) { self.accepted.push_back(session); }
    pub fn try_accept(&mut self) -> Stream<WtSession<C>, ()> {
        if let Some(s) = self.accepted.pop_front() { Stream::Next(s) } else { Stream::Pending(()) }
    }
    pub fn pending(&self) -> usize { self.accepted.len() }

    /// Build 200 response headers for Extended CONNECT (QPACK-encoded by caller).
    pub fn build_connect_response_headers() -> Vec<(Vec<u8>, Vec<u8>)> {
        vec![
            (b":status".to_vec(), b"200".to_vec()),
            (b"sec-webtransport-http3-draft".to_vec(), b"draft-07".to_vec()),
        ]
    }
}

impl<C: QuicConnection> Default for WtAcceptor<C> {
    fn default() -> Self { Self::new() }
}

// ── WtConnector ──
pub struct WtConnector;

impl WtConnector {
    /// Build request headers for Extended CONNECT (QPACK-encoded by caller).
    pub fn build_connect_headers(authority: &str, path: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
        vec![
            (b":method".to_vec(), b"CONNECT".to_vec()),
            (b":protocol".to_vec(), b"webtransport".to_vec()),
            (b":scheme".to_vec(), b"https".to_vec()),
            (b":authority".to_vec(), authority.as_bytes().to_vec()),
            (b":path".to_vec(), path.as_bytes().to_vec()),
            (b"sec-webtransport-http3-draft".to_vec(), b"draft-07".to_vec()),
        ]
    }

    /// Wrap an existing QUIC connection as a WebTransport session
    /// (client-side — call after the Extended CONNECT handshake completes).
    ///
    /// The QUIC connection has already completed the CONNECT → 200 OK
    /// exchange (driven by http3 or manual framing). This constructs a
    /// [`WtSession`] that exposes streams and datagrams.
    #[must_use]
    pub fn into_session<C: crate::quic::QuicConnection>(
        conn: C,
        datagrams: bool,
    ) -> WtSession<C> {
        WtSession::new(conn, datagrams)
    }
}

// ── NoIoSession — sans-I/O, no QUIC connection ──
pub struct NoIoSession {
    pub state: WtSessionState,
    datagrams_enabled: bool,
    recv_datagrams: VecDeque<Bytes>,
    peer_close: Option<(u32, String)>,
    send_datagrams: VecDeque<Bytes>,
}

impl NoIoSession {
    pub fn new(_is_client: bool, datagrams_enabled: bool) -> Self {
        Self {
            state: WtSessionState::Connecting,
            datagrams_enabled,
            recv_datagrams: VecDeque::new(),
            peer_close: None,
            send_datagrams: VecDeque::new(),
        }
    }
    pub fn on_connected(&mut self) { self.state = WtSessionState::Open; }
    pub fn on_capsule(&mut self, capsule: CapsuleType) -> bool {
        match capsule {
            CapsuleType::Datagram(data) => {
                if self.datagrams_enabled { self.recv_datagrams.push_back(Bytes::from(data)); }
                false
            }
            CapsuleType::CloseSession { code, reason } => { self.peer_close = Some((code, reason)); self.state = WtSessionState::Closed; true }
            CapsuleType::Drain => { self.state = WtSessionState::Draining; true }
            CapsuleType::Unknown(_, _) => false,
        }
    }
    pub fn is_open(&self) -> bool { self.state == WtSessionState::Open }
    pub fn is_closed(&self) -> bool { self.state == WtSessionState::Closed }
    pub fn close_info(&self) -> Option<(u32, &str)> { self.peer_close.as_ref().map(|(c,r)| (*c, r.as_str())) }
    pub fn datagrams_enabled(&self) -> bool { self.datagrams_enabled }
    pub fn queue_datagram(&mut self, data: Bytes) -> Result<(), WtStreamError> {
        if !self.datagrams_enabled { return Err(WtStreamError::DatagramsNotSupported); }
        if self.is_closed() { return Err(WtStreamError::Closed(WtProtocolError::SessionClosed { code: 0, reason: String::new() })); }
        self.send_datagrams.push_back(data);
        Ok(())
    }
    pub fn drain_send_datagrams(&mut self) -> Vec<Bytes> { self.send_datagrams.drain(..).collect() }
    pub fn try_recv_datagram(&mut self) -> Stream<Result<Option<Bytes>, WtStreamError>, ()> {
        if let Some((c, r)) = &self.peer_close { return Stream::Next(Err(WtStreamError::Closed(WtProtocolError::SessionClosed { code: *c, reason: r.clone() }))); }
        if let Some(d) = self.recv_datagrams.pop_front() { Stream::Next(Ok(Some(d))) } else { Stream::Pending(()) }
    }
}
