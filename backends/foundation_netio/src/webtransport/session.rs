//! WebTransport session: accept/connect, streams, datagrams, close (spec-55, F06).
//!
//! WHY: The session is the unit of WebTransport — one Extended CONNECT handshake
//! establishes a session, then bidi/uni streams and datagrams flow independently.
//!
//! WHAT: [`WtSession`] — open/accept bidirectional and unidirectional streams,
//! send/receive datagrams, close. [`WtAcceptor`] — server-side accept loop.
//! [`WtConnector`] — client-side connect.
//!
//! HOW: All I/O is progress-returning (`Stream`). The caller's valtron task drives
//! the session by calling methods in a loop and mapping `Pending`/`Wait` to task
//! readiness. Streams are a passthrough to the underlying QUIC connection — the
//! only WebTransport-specific work is the Extended CONNECT handshake and capsule
//! processing on the session control stream.

use std::collections::VecDeque;

use bytes::Bytes;
use foundation_core::valtron::Stream;

use super::proto::{self, CapsuleType, WtProtocolError};

// ---------------------------------------------------------------------------
// WtStreamError
// ---------------------------------------------------------------------------

/// Errors surfaced by WebTransport session methods.
#[derive(Debug)]
pub enum WtStreamError {
    /// The session was closed or rejected.
    Closed(WtProtocolError),
    /// An underlying transport error.
    Transport(String),
    /// Datagrams are not supported by the QUIC backend.
    DatagramsNotSupported,
}

impl std::fmt::Display for WtStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed(e) => write!(f, "session closed: {e}"),
            Self::Transport(msg) => write!(f, "transport error: {msg}"),
            Self::DatagramsNotSupported => write!(f, "datagrams not supported"),
        }
    }
}

impl std::error::Error for WtStreamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Closed(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// WtSession — one WebTransport session
// ---------------------------------------------------------------------------

/// State of a WebTransport session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WtSessionState {
    /// Extended CONNECT is in progress (or not yet started).
    Connecting,
    /// Session is open; streams and datagrams can be used.
    Open,
    /// The peer sent DRAIN — stop creating new streams, drain existing ones.
    Draining,
    /// Session is closed (clean shutdown or error).
    Closed,
}

/// A WebTransport session over an established HTTP/3 connection.
///
/// WHY: After the Extended CONNECT handshake, the session provides multiplexed
/// bidi/uni streams + optional unreliable datagrams over the QUIC connection.
///
/// WHAT: Methods for opening and accepting streams, sending and receiving
/// datagrams, and closing the session.
///
/// HOW: Streams are direct pass-through to the QUIC connection — a WebTransport
/// stream IS a QUIC stream with type prefix `0x54`. Datagrams are QUIC datagrams.
/// The session control stream (the CONNECT stream) carries capsule frames for
/// session lifecycle events.
pub struct WtSession {
    /// Current session state.
    pub state: WtSessionState,
    /// Whether the underlying QUIC backend supports datagrams (RFC 9221).
    datagrams_enabled: bool,
    /// Whether this session was opened by us (client) or accepted (server).
    _is_client: bool,
    /// Buffered inbound datagrams.
    recv_datagrams: VecDeque<Bytes>,
    /// Whether the session has been closed by the peer (capsule received).
    peer_close: Option<(u32, String)>,
    /// Outbound datagrams queued for flush.
    send_datagrams: VecDeque<Bytes>,
}

impl WtSession {
    /// WHY: Server accept or client connect returns a fresh session.
    ///
    /// WHAT: Create a new session in `Connecting` state.
    #[must_use]
    pub fn new(is_client: bool, datagrams_enabled: bool) -> Self {
        Self {
            state: WtSessionState::Connecting,
            datagrams_enabled,
            _is_client: is_client,
            recv_datagrams: VecDeque::new(),
            peer_close: None,
            send_datagrams: VecDeque::new(),
        }
    }

    /// Transition the session to `Open` once Extended CONNECT succeeds.
    pub fn on_connected(&mut self) {
        self.state = WtSessionState::Open;
    }

    /// Process a capsule frame received on the session control stream.
    ///
    /// Returns `true` if the session is now closed or draining.
    #[must_use]
    pub fn on_capsule(&mut self, capsule: CapsuleType) -> bool {
        match capsule {
            CapsuleType::Datagram(data) => {
                if self.datagrams_enabled {
                    self.recv_datagrams.push_back(Bytes::from(data));
                }
                false
            }
            CapsuleType::CloseSession { code, reason } => {
                self.peer_close = Some((code, reason));
                self.state = WtSessionState::Closed;
                true
            }
            CapsuleType::Drain => {
                self.state = WtSessionState::Draining;
                true
            }
            CapsuleType::Unknown(_, _) => false,
        }
    }

    /// Whether the session is open (streams and datagrams can be used).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state == WtSessionState::Open
    }

    /// Whether the session is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == WtSessionState::Closed
    }

    /// The close code and reason if the peer closed the session, or our own.
    #[must_use]
    pub fn close_info(&self) -> Option<(u32, &str)> {
        self.peer_close
            .as_ref()
            .map(|(c, r)| (*c, r.as_str()))
    }

    /// Encode the close session capsule for sending to the peer.
    #[must_use]
    pub fn build_close_capsule(code: u32, reason: &str) -> Vec<u8> {
        proto::encode_close_session(code, reason)
    }

    // ── Datagram API ──

    /// Whether the QUIC backend supports datagrams.
    #[must_use]
    pub fn datagrams_enabled(&self) -> bool {
        self.datagrams_enabled
    }

    /// Queue an outbound datagram. Returns an error if datagrams are not enabled
    /// or the session is closed.
    pub fn queue_datagram(&mut self, data: Bytes) -> Result<(), WtStreamError> {
        if !self.datagrams_enabled {
            return Err(WtStreamError::DatagramsNotSupported);
        }
        if self.is_closed() {
            return Err(WtStreamError::Closed(WtProtocolError::SessionClosed {
                code: 0,
                reason: String::new(),
            }));
        }
        self.send_datagrams.push_back(data);
        Ok(())
    }

    /// Drain all pending outbound datagrams for the caller to feed to the QUIC layer.
    pub fn drain_send_datagrams(&mut self) -> Vec<Bytes> {
        self.send_datagrams.drain(..).collect()
    }

    /// Try to receive a datagram. Returns `Stream::Next(Some(data))` if one is
    /// available, `Stream::Pending(())` if none are queued, or
    /// `Stream::Next(Err(...))` on session close.
    pub fn try_recv_datagram(&mut self) -> Stream<Result<Option<Bytes>, WtStreamError>, ()> {
        if let Some((code, reason)) = &self.peer_close {
            return Stream::Next(Err(WtStreamError::Closed(WtProtocolError::SessionClosed {
                code: *code,
                reason: reason.clone(),
            })));
        }
        if let Some(data) = self.recv_datagrams.pop_front() {
            Stream::Next(Ok(Some(data)))
        } else {
            Stream::Pending(())
        }
    }
}

// ---------------------------------------------------------------------------
// WtAcceptor — server-side
// ---------------------------------------------------------------------------

/// Server-side WebTransport session acceptor.
///
/// WHY: A server wants to accept inbound WebTransport sessions from HTTP/3
/// Extended CONNECT requests.
///
/// WHAT: Wraps an H3 connection. `accept()` returns a [`WtSession`] when an
/// incoming WebTransport request arrives.
///
/// HOW: The caller drives the accept loop: read inbound bidirectional streams,
/// check for the `:protocol = webtransport` pseudo-header, perform the Extended
/// CONNECT handshake, and return a session.
pub struct WtAcceptor {
    /// Accepted sessions awaiting pickup by the application.
    accepted: VecDeque<WtSession>,
}

impl WtAcceptor {
    /// Create a new acceptor with no queued sessions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accepted: VecDeque::new(),
        }
    }

    /// WHY: The accept loop calls this after completing an Extended CONNECT
    /// handshake to queue a ready session.
    ///
    /// WHAT: Push a successfully-established session into the accept queue.
    pub fn queue_session(&mut self, session: WtSession) {
        self.accepted.push_back(session);
    }

    /// Try to accept a session. Returns `Stream::Next(session)` if one is
    /// available, `Stream::Pending(())` if none are queued yet.
    pub fn try_accept(&mut self) -> Stream<WtSession, ()> {
        if let Some(session) = self.accepted.pop_front() {
            Stream::Next(session)
        } else {
            Stream::Pending(())
        }
    }

    /// Build an Extended CONNECT accept response.
    ///
    /// After receiving a request with `:protocol = webtransport` on a bidi stream,
    /// the server sends back `200 OK` on that same stream. The stream then becomes
    /// the session control stream for capsule frames.
    #[must_use]
    pub fn build_connect_response() -> Vec<u8> {
        // Minimal HTTP/3 200 response for Extended CONNECT.
        // In a real implementation this would go through the h3 framing layer.
        b"HTTP/3 200 OK\r\nsec-webtransport-http3-draft: draft-07\r\n\r\n".to_vec()
    }

    /// Number of queued sessions.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.accepted.len()
    }
}

impl Default for WtAcceptor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WtConnector — client-side
// ---------------------------------------------------------------------------

/// Client-side WebTransport connector.
///
/// WHY: A client initiates a WebTransport session by sending an Extended CONNECT
/// request on a new bidi stream.
///
/// WHAT: Builds the CONNECT request, processes the server response, and returns
/// a [`WtSession`].
pub struct WtConnector;

impl WtConnector {
    /// Build an Extended CONNECT request for WebTransport.
    ///
    /// The request is sent on a new bidi stream opened by the client. After the
    /// server responds with 200, that stream becomes the session control stream.
    #[must_use]
    pub fn build_connect_request(authority: &str, path: &str) -> Vec<u8> {
        let request = format!(
            "CONNECT {path} HTTP/3\r\n\
             :authority: {authority}\r\n\
             :protocol: webtransport\r\n\
             sec-webtransport-http3-draft: draft-07\r\n\
             \r\n"
        );
        request.into_bytes()
    }
}

// ---------------------------------------------------------------------------
// Tests
