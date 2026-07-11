//! Shared connection state (F33).
//!
//! WHY: `quinn_proto::Connection` hands out streams as **short-lived borrows** —
//! `recv_stream(&mut self, id) -> RecvStream<'_>`. So a `QuicRecvStream` handle
//! cannot own a `&mut Connection`; it can only own a way to *re-borrow* one. And
//! the driver task, which pumps datagrams and timers, needs the same connection at
//! the same time.
//!
//! WHAT: [`ConnState`] — the connection state machine, its peer, and the accept
//! queues — behind one `Arc<Mutex<_>>` shared by the driver and every stream
//! handle. The endpoint and socket live in the driver: one endpoint routes for
//! many connections.
//!
//! HOW: each trait call locks, re-borrows the `quinn_proto::Connection` for the
//! duration of one operation, and returns a single `Stream<..>` value. Locks are
//! never held across a blocking call: `quinn-proto` is sans-IO, so nothing under
//! this lock can block. The only I/O is in the driver, which does it outside the
//! borrow.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use quinn_proto::{ConnectionHandle, Dir};

use super::traits::QuicConnError;
use super::StreamId;

/// Everything one QUIC connection needs, shared between its driver task and its
/// stream handles.
pub(crate) struct ConnState {
    /// This connection's handle within the endpoint that owns it.
    ///
    /// The endpoint itself lives in the driver, not here: one endpoint routes
    /// datagrams for *many* connections, so it cannot be per-connection state.
    #[allow(dead_code)]
    pub(crate) handle: ConnectionHandle,
    /// The sans-IO connection state machine.
    pub(crate) conn: quinn_proto::Connection,
    /// The peer we are talking to.
    pub(crate) peer: SocketAddr,
    /// Peer-initiated bidirectional streams the driver has accepted, awaiting a
    /// `QuicConnection::accept_bidi` caller.
    pub(crate) inbound_bidi: VecDeque<StreamId>,
    /// Peer-initiated unidirectional streams, awaiting `accept_recv`.
    pub(crate) inbound_uni: VecDeque<StreamId>,
    /// Set once the connection has ended; every subsequent trait call reports it
    /// rather than pretending the connection is merely idle.
    pub(crate) closed: Option<QuicConnError>,
}

impl ConnState {
    /// Whether the handshake has completed and streams may be used.
    pub(crate) fn is_established(&self) -> bool {
        !self.conn.is_handshaking()
    }

    /// Take the next inbound stream of the given direction, if the driver has
    /// queued one.
    pub(crate) fn next_inbound(&mut self, dir: Dir) -> Option<StreamId> {
        match dir {
            Dir::Bi => self.inbound_bidi.pop_front(),
            Dir::Uni => self.inbound_uni.pop_front(),
        }
    }

    /// Queue an inbound stream the driver accepted from `quinn_proto`.
    pub(crate) fn push_inbound(&mut self, dir: Dir, id: StreamId) {
        match dir {
            Dir::Bi => self.inbound_bidi.push_back(id),
            Dir::Uni => self.inbound_uni.push_back(id),
        }
    }
}

/// A handle to [`ConnState`], cloned into the driver and every stream.
pub(crate) type SharedConn = Arc<Mutex<ConnState>>;

/// Lock the shared state, converting a poisoned mutex into a connection error
/// rather than panicking a transport task.
pub(crate) fn lock(
    state: &SharedConn,
) -> Result<std::sync::MutexGuard<'_, ConnState>, QuicConnError> {
    state
        .lock()
        .map_err(|_| QuicConnError::Internal("QUIC connection state lock poisoned".into()))
}
