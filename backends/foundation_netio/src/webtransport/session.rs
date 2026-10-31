//! WebTransport session: accept/connect, streams, datagrams, close (spec-55, F06).

use std::collections::VecDeque;

use bytes::{Buf, Bytes};
use foundation_core::valtron::Stream;

#[cfg(feature = "quic")]
use crate::quic::{
    QuicConnError, QuicConnection, QuicRecvStream, QuicSendStream, QuicStreamError,
};

use super::proto::{self, CapsuleDecoder, CapsuleType, WtProtocolError};

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
    /// The CONNECT stream, once the session is established. Its close ends the
    /// session (draft-ietf-webtrans-http3 §5). `None` on the connector path,
    /// where the caller tracks the stream itself.
    connect: Option<C::BidiStream>,
    /// Decodes capsules the peer sends on the CONNECT stream.
    capsules: CapsuleDecoder,
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
            connect: None,
            capsules: CapsuleDecoder::new(),
        }
    }

    /// An accepted session holding its open CONNECT stream. The session lives as
    /// long as that stream stays open (draft-ietf-webtrans-http3 §5).
    pub fn new_with_connect(conn: C, connect: C::BidiStream, datagrams_enabled: bool) -> Self {
        let mut session = Self::new(conn, datagrams_enabled);
        session.connect = Some(connect);
        session
    }

    /// Drive the CONNECT stream: process capsules and detect the session end.
    ///
    /// WHY: the session's lifetime is the CONNECT stream's. A FIN, a reset, or a
    /// `CLOSE_WEBTRANSPORT_SESSION` capsule ends it (draft-ietf-webtrans-http3 §5).
    ///
    /// WHAT: `Next(Ok(true))` when the session has ended; `Next(Ok(false))` after
    /// progress that did not end it; `Pending` when idle; `Err` on a transport or
    /// capsule failure. A caller whose `poll_*` returned `Pending` parks rather
    /// than re-polling.
    ///
    /// HOW: read the CONNECT stream, decode capsules, and reflect the result into
    /// the session state.
    pub fn poll_connect(&mut self) -> Stream<Result<bool, WtStreamError>, ()> {
        loop {
            // Drain any capsule already buffered.
            match self.capsules.decode() {
                Ok(Some(capsule)) => {
                    let _ = self.on_capsule(capsule);
                    if self.is_closed() {
                        return Stream::Next(Ok(true));
                    }
                    continue;
                }
                Ok(None) => {}
                Err(e) => return Stream::Next(Err(WtStreamError::Transport(e.to_string()))),
            }

            // Read the CONNECT stream.
            let Some(connect) = self.connect.as_mut() else {
                return Stream::Pending(());
            };
            match connect.read() {
                Stream::Next(Ok(Some(bytes))) => {
                    self.capsules.push(&bytes);
                    continue;
                }
                // A clean FIN or a reset both end the session (draft §5).
                Stream::Next(Ok(None)) => {
                    self.state = WtSessionState::Closed;
                    return Stream::Next(Ok(true));
                }
                Stream::Next(Err(_)) => {
                    self.state = WtSessionState::Closed;
                    return Stream::Next(Ok(true));
                }
                _ => return Stream::Pending(()),
            }
        }
    }

    pub fn on_connected(&mut self) { self.state = WtSessionState::Open; }

    pub fn on_capsule(&mut self, capsule: CapsuleType) -> bool {
        match capsule {
            CapsuleType::Datagram(data) => {
                if self.datagrams_enabled { self.recv_datagrams.push_back(Bytes::from(data)); }
                false
            }
            CapsuleType::CloseWebTransportSession { code, reason } => {
                self.peer_close = Some((code, reason));
                self.state = WtSessionState::Closed;
                true
            }
            CapsuleType::Drain => { self.state = WtSessionState::Draining; true }
            CapsuleType::Grease { .. } => false,
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

    /// Close the session: send a `CLOSE_WEBTRANSPORT_SESSION` capsule and then FIN
    /// the CONNECT stream (draft-ietf-webtrans-http3 §5). Without a tracked CONNECT
    /// stream, close the whole QUIC connection instead.
    ///
    /// `Pending` until the capsule and FIN are flushed.
    pub fn close(&mut self, code: u32, reason: &str) -> Stream<Result<(), WtStreamError>, ()> {
        self.state = WtSessionState::Closed;

        let Some(connect) = self.connect.as_mut() else {
            self.conn.close(code as u64, reason.as_bytes());
            return Stream::Next(Ok(()));
        };

        let mut capsule = Bytes::from(Self::build_close_capsule(code, reason));
        loop {
            if !capsule.has_remaining() {
                break;
            }
            match connect.send(&mut capsule) {
                Stream::Next(Ok(_)) => {}
                Stream::Next(Err(e)) => {
                    return Stream::Next(Err(WtStreamError::Transport(e.to_string())))
                }
                _ => return Stream::Pending(()),
            }
        }

        match connect.finish() {
            Stream::Next(Ok(())) => Stream::Next(Ok(())),
            Stream::Next(Err(e)) => Stream::Next(Err(WtStreamError::Transport(e.to_string()))),
            _ => Stream::Pending(()),
        }
    }
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

/// Error during the Extended CONNECT handshake (client side).
/// Matches `ConnectError` from the reference `web-transport-proto`.
#[derive(Debug)]
pub enum WtConnectError {
    /// The server rejected the CONNECT request (non-200 status).
    Rejected(String),
    /// The QUIC connection or stream failed.
    Quic(String),
    /// The peer closed before responding.
    Closed,
}

impl std::fmt::Display for WtConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(m) => write!(f, "CONNECT rejected: {m}"),
            Self::Quic(m) => write!(f, "QUIC error: {m}"),
            Self::Closed => write!(f, "connection closed during CONNECT"),
        }
    }
}
impl std::error::Error for WtConnectError {}

/// Utility for building WebTransport client sessions.
pub struct WtConnector;

impl WtConnector {
    /// Build request headers for Extended CONNECT.
    ///
    /// These are the HTTP pseudo-headers QPACK encodes for the CONNECT
    /// request. The caller serializes them into QPACK format and sends
    /// them on an http3 bidi stream.
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

    /// Check if an HTTP/3 response is a successful WebTransport CONNECT
    /// response (status 200). Matches `ConnectRequest::check_response`
    /// from the reference.
    #[must_use]
    pub fn is_success_response(headers: &[(Vec<u8>, Vec<u8>)]) -> bool {
        headers
            .iter()
            .any(|(k, v)| k == b":status" && v == b"200")
    }

    /// Wrap an existing QUIC connection as a WebTransport session
    /// (client-side — call after the Extended CONNECT handshake completes).
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
            CapsuleType::CloseWebTransportSession { code, reason } => { self.peer_close = Some((code, reason)); self.state = WtSessionState::Closed; true }
            CapsuleType::Drain => { self.state = WtSessionState::Draining; true }
            CapsuleType::Grease { .. } => false,
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
