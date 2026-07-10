//! QUIC transport layer — trait abstraction + quinn-proto driver (F33).
//!
//! WHY: HTTP/3 needs a QUIC backend, and must not be welded to one. `quinn-proto`
//! is a sans-IO state machine — no tokio, no async runtime, no I/O of its own. We
//! supply the UDP plumbing and drive it from a valtron task.
//!
//! WHAT:
//! - [`traits`] — the backend-agnostic trait set HTTP/3 is written against
//!   ([`QuicConnection`], [`QuicSendStream`], [`QuicRecvStream`], [`QuicBidiStream`]).
//!   Progress-returning and synchronous; no `Poll`, no `Waker`, no futures.
//! - [`QuicDriver`] — the valtron task that pumps one `quinn_proto::Connection`:
//!   datagrams in, `poll_transmit` out, timers, and connection events. Parks on the
//!   native reactor via `TaskStatus::Depends(fd_readiness)` rather than busy-polling.
//! - [`StreamId`] — a backend-neutral stream identifier, so the traits never leak
//!   `quinn_proto` types to HTTP/3.
//!
//! HOW: `quinn_proto::Connection` hands out streams as **short-lived borrows**
//! (`recv_stream(&mut self, id) -> RecvStream<'_>`), so a stream handle cannot own
//! a `&mut Connection`. The driver and every stream handle instead share one
//! `Arc<Mutex<ConnState>>` and carry a [`StreamId`]; each trait call locks, acts,
//! and returns a single `Stream<..>` value.

/// The backend-agnostic QUIC trait set (Decision 01, normative).
pub mod traits;

pub use traits::{QuicBidiStream, QuicConnError, QuicConnection, QuicRecvStream, QuicSendStream, QuicStreamError};

#[cfg(feature = "quic")]
mod state;

#[cfg(feature = "quic")]
mod quinn_impl;

#[cfg(feature = "quic")]
mod quinn_driver;

#[cfg(feature = "quic")]
pub use quinn_driver::{
    client_config_trusting, client_config_trusting_pem, server_config_from_der,
    server_config_from_pem, QuicDriver, QuicEvent,
};

#[cfg(feature = "quic")]
pub use quinn_impl::{QuinnBidiStream, QuinnConnection, QuinnRecvStream, QuinnSendStream};

/// QUIC configuration, re-exported so callers need no `quinn-proto` dependency of
/// their own. `H3Transport` takes a [`ClientConfig`]; leaking `quinn_proto` into a
/// downstream crate's public API would force that crate to depend on it.
#[cfg(feature = "quic")]
pub use quinn_proto::{ClientConfig, ServerConfig};

/// A QUIC stream identifier.
///
/// WHY: the trait set is backend-agnostic, so it must not expose
/// `quinn_proto::StreamId`. HTTP/3 also cares about the low bits — they encode the
/// initiator and the directionality (RFC 9000 §2.1).
///
/// WHAT: the 62-bit stream id, with the RFC 9000 accessors HTTP/3 needs.
///
/// HOW: bit 0 is the initiator (0 = client, 1 = server); bit 1 is the direction
/// (0 = bidirectional, 1 = unidirectional).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(u64);

impl StreamId {
    /// Wrap a raw 62-bit stream id.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw 62-bit value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Was this stream opened by the client? (RFC 9000 §2.1, bit 0)
    #[must_use]
    pub const fn is_client_initiated(self) -> bool {
        self.0 & 0b01 == 0
    }

    /// Was this stream opened by the server?
    #[must_use]
    pub const fn is_server_initiated(self) -> bool {
        !self.is_client_initiated()
    }

    /// Is this a bidirectional stream? (RFC 9000 §2.1, bit 1)
    #[must_use]
    pub const fn is_bidirectional(self) -> bool {
        self.0 & 0b10 == 0
    }

    /// Is this a unidirectional stream?
    #[must_use]
    pub const fn is_unidirectional(self) -> bool {
        !self.is_bidirectional()
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for StreamId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<StreamId> for u64 {
    fn from(id: StreamId) -> Self {
        id.0
    }
}

#[cfg(feature = "quic")]
impl From<quinn_proto::StreamId> for StreamId {
    fn from(id: quinn_proto::StreamId) -> Self {
        Self(id.into())
    }
}

/// A stream id that does not fit QUIC's 62-bit varint encoding.
///
/// Only reachable by constructing a [`StreamId`] from a raw `u64` above `2^62 - 1`.
/// Ids that came from the backend can never trip it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamIdOutOfRange(u64);

impl std::fmt::Display for StreamIdOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stream id {} exceeds QUIC's 62-bit varint range", self.0)
    }
}

impl std::error::Error for StreamIdOutOfRange {}

/// WHY: `quinn_proto::StreamId` is only constructible from a `VarInt`, and a
/// `VarInt` is only 62 bits. Rather than `expect` our way past that, the
/// conversion is fallible — an out-of-range id is a caller error, not a panic.
#[cfg(feature = "quic")]
impl TryFrom<StreamId> for quinn_proto::StreamId {
    type Error = StreamIdOutOfRange;

    fn try_from(id: StreamId) -> Result<Self, Self::Error> {
        quinn_proto::VarInt::from_u64(id.0)
            .map(quinn_proto::StreamId::from)
            .map_err(|_| StreamIdOutOfRange(id.0))
    }
}
