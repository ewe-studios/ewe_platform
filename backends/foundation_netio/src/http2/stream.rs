//! HTTP/2 stream state machine (RFC 7540 §5.1).
//!
//! WHY: Every HTTP/2 stream transitions through a well-defined life cycle
//! (idle → open → half-closed → closed). Both endpoints must track this
//! independently, and illegal transitions (e.g. sending DATA on a closed
//! stream) must be detected as protocol errors.
//!
//! WHAT: [`StreamState`] — the 6 legal states plus the implicit `closed`
//! terminal. Validated transition functions for each frame type.
//!
//! HOW: Pure state machine, no I/O. Callers query legal transitions and
//! apply them after validating the frame.

/// The 6 active stream states (RFC 7540 §5.1). `Closed` is implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Initial state — no frames exchanged yet.
    Idle,
    /// HEADERS sent (server push: PUSH_PROMISE sent on associated stream).
    ReservedLocal,
    /// HEADERS received (server push: PUSH_PROMISE received).
    ReservedRemote,
    /// Both endpoints can send any frame type.
    Open,
    /// The local endpoint sent END_STREAM.
    HalfClosedLocal,
    /// The remote endpoint sent END_STREAM.
    HalfClosedRemote,
    /// Terminal — RST_STREAM sent/received, or both sides ended.
    Closed,
}

/// Direction for half-close.
#[derive(Debug, Clone, Copy)]
pub enum StreamSide {
    Local,
    Remote,
}

impl StreamState {
    /// The initial state for a new client-initiated stream (odd stream ID).
    #[must_use]
    pub fn new_client_initiated() -> Self {
        StreamState::Idle
    }

    /// The initial state for a server-pushed stream (even stream ID).
    /// These start in `ReservedLocal` — the server already "sent" the push promise
    /// on the associated stream, so the new stream enters reserved state.
    #[must_use]
    pub fn new_server_push() -> Self {
        StreamState::ReservedLocal
    }

    // ── Send transitions ─────────────────────────────────────────────────

    /// We are sending HEADERS (or CONTINUATION). Transitions:
    /// - Idle → Open (or HalfClosedLocal if END_STREAM)
    /// - ReservedLocal → HalfClosedRemote (or Closed if END_STREAM)
    /// - Open → Open (trailers/no-op; → HalfClosedLocal if END_STREAM)
    ///
    /// # Errors
    /// Returns the current state if this transition is illegal.
    pub fn send_headers(&mut self, end_stream: bool) -> Result<(), StreamState> {
        match *self {
            StreamState::Idle => {
                *self = if end_stream { StreamState::HalfClosedLocal } else { StreamState::Open };
                Ok(())
            }
            StreamState::ReservedLocal => {
                *self = if end_stream { StreamState::Closed } else { StreamState::HalfClosedRemote };
                Ok(())
            }
            StreamState::Open => {
                if end_stream { *self = StreamState::HalfClosedLocal; }
                Ok(())
            }
            _ => Err(*self),
        }
    }

    /// We are sending DATA. Only valid in Open or HalfClosedRemote.
    ///
    /// # Errors
    pub fn send_data(&mut self, end_stream: bool) -> Result<(), StreamState> {
        match *self {
            StreamState::Open => {
                if end_stream { *self = StreamState::HalfClosedLocal; }
                Ok(())
            }
            StreamState::HalfClosedRemote => {
                if end_stream { *self = StreamState::Closed; }
                Ok(())
            }
            _ => Err(*self),
        }
    }

    /// We are sending RST_STREAM. Closes the stream from any state except Idle.
    ///
    /// # Errors
    pub fn send_reset(&mut self) -> Result<(), StreamState> {
        match *self {
            StreamState::Idle => Err(StreamState::Idle),
            _ => { *self = StreamState::Closed; Ok(()) }
        }
    }

    /// We are sending a PUSH_PROMISE on an associated stream.
    /// Creates a new promised stream in ReservedLocal.
    #[must_use]
    pub fn send_push_promise(&self) -> Result<StreamState, StreamState> {
        match *self {
            StreamState::Open | StreamState::HalfClosedRemote => Ok(StreamState::ReservedLocal),
            _ => Err(*self),
        }
    }

    // ── Receive transitions ──────────────────────────────────────────────

    /// We received HEADERS (or CONTINUATION). Transitions:
    /// - Idle → Open (or HalfClosedRemote if END_STREAM)
    /// - ReservedRemote → HalfClosedLocal (or Closed if END_STREAM)
    /// - Open → Open (trailers; → HalfClosedRemote if END_STREAM)
    /// - HalfClosedLocal → HalfClosedLocal (trailers; → Closed if END_STREAM)
    ///
    /// # Errors
    pub fn recv_headers(&mut self, end_stream: bool) -> Result<(), StreamState> {
        match *self {
            StreamState::Idle => {
                *self = if end_stream { StreamState::HalfClosedRemote } else { StreamState::Open };
                Ok(())
            }
            StreamState::ReservedRemote => {
                *self = if end_stream { StreamState::Closed } else { StreamState::HalfClosedLocal };
                Ok(())
            }
            StreamState::Open => {
                if end_stream { *self = StreamState::HalfClosedRemote; }
                Ok(())
            }
            StreamState::HalfClosedLocal => {
                if end_stream { *self = StreamState::Closed; }
                Ok(())
            }
            _ => Err(*self),
        }
    }

    /// We received DATA. Only valid in Open or HalfClosedLocal.
    ///
    /// # Errors
    pub fn recv_data(&mut self, end_stream: bool) -> Result<(), StreamState> {
        match *self {
            StreamState::Open => {
                if end_stream { *self = StreamState::HalfClosedRemote; }
                Ok(())
            }
            StreamState::HalfClosedLocal => {
                if end_stream { *self = StreamState::Closed; }
                Ok(())
            }
            _ => Err(*self),
        }
    }

    /// We received RST_STREAM. Closes the stream from any state except Idle.
    ///
    /// # Errors
    pub fn recv_reset(&mut self) -> Result<(), StreamState> {
        match *self {
            StreamState::Idle => Err(StreamState::Idle),
            _ => { *self = StreamState::Closed; Ok(()) }
        }
    }

    /// We received a PUSH_PROMISE. The new promised stream enters ReservedRemote.
    #[must_use]
    pub fn recv_push_promise(&self) -> Result<StreamState, StreamState> {
        match *self {
            StreamState::Open | StreamState::HalfClosedLocal => Ok(StreamState::ReservedRemote),
            _ => Err(*self),
        }
    }

    /// Check if this stream can send frames.
    #[must_use]
    pub fn can_send(&self) -> bool {
        matches!(*self, StreamState::Open | StreamState::HalfClosedRemote | StreamState::ReservedLocal)
    }

    /// Check if this stream can receive frames.
    #[must_use]
    pub fn can_recv(&self) -> bool {
        matches!(*self, StreamState::Open | StreamState::HalfClosedLocal | StreamState::ReservedRemote)
    }
}
