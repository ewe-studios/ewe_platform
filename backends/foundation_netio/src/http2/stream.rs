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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Client-initiated (odd ID) life cycle ────────────────────────────

    #[test]
    fn client_stream_idle_to_open_to_closed() {
        let mut s = StreamState::Idle;

        // Client sends HEADERS → Open
        assert!(s.send_headers(false).is_ok());
        assert_eq!(s, StreamState::Open);

        // Client sends DATA with END_STREAM → HalfClosedLocal
        assert!(s.send_data(true).is_ok());
        assert_eq!(s, StreamState::HalfClosedLocal);

        // Server sends HEADERS with END_STREAM → Closed
        assert!(s.recv_headers(true).is_ok());
        assert_eq!(s, StreamState::Closed);
    }

    #[test]
    fn server_side_idle_to_open_to_closed() {
        let mut s = StreamState::Idle;

        // Server receives HEADERS → Open
        assert!(s.recv_headers(false).is_ok());
        assert_eq!(s, StreamState::Open);

        // Server sends HEADERS with END_STREAM → HalfClosedLocal
        assert!(s.send_headers(true).is_ok());
        assert_eq!(s, StreamState::HalfClosedLocal);

        // Client sends DATA with END_STREAM → Closed
        assert!(s.recv_data(true).is_ok());
        assert_eq!(s, StreamState::Closed);
    }

    // ── Illegal transitions ──────────────────────────────────────────────

    #[test]
    fn send_data_on_idle_is_error() {
        let mut s = StreamState::Idle;
        assert_eq!(s.send_data(false), Err(StreamState::Idle));
    }

    #[test]
    fn recv_data_on_idle_is_error() {
        let mut s = StreamState::Idle;
        assert_eq!(s.recv_data(false), Err(StreamState::Idle));
    }

    #[test]
    fn send_headers_on_closed_is_error() {
        let mut s = StreamState::Closed;
        assert_eq!(s.send_headers(false), Err(StreamState::Closed));
    }

    #[test]
    fn reset_on_idle_is_error() {
        let mut s = StreamState::Idle;
        assert_eq!(s.send_reset(), Err(StreamState::Idle));
        assert_eq!(s.recv_reset(), Err(StreamState::Idle));
    }

    // ── RST_STREAM terminates ────────────────────────────────────────────

    #[test]
    fn reset_from_open_closes() {
        let mut s = StreamState::Open;
        assert!(s.send_reset().is_ok());
        assert_eq!(s, StreamState::Closed);
    }

    #[test]
    fn reset_from_half_closed_closes() {
        let mut s = StreamState::HalfClosedLocal;
        assert!(s.recv_reset().is_ok());
        assert_eq!(s, StreamState::Closed);
    }

    // ── Server push ──────────────────────────────────────────────────────

    #[test]
    fn push_promise_from_open() {
        let s = StreamState::Open;
        let promised = s.send_push_promise().unwrap();
        assert_eq!(promised, StreamState::ReservedLocal);
    }

    #[test]
    fn push_promise_received() {
        let s = StreamState::Open;
        let promised = s.recv_push_promise().unwrap();
        assert_eq!(promised, StreamState::ReservedRemote);
    }

    #[test]
    fn push_promise_from_idle_is_error() {
        let s = StreamState::Idle;
        assert!(s.send_push_promise().is_err());
    }

    // ── Full matrix ──────────────────────────────────────────────────────

    #[test]
    fn all_states_exhaustive() {
        // Verify that every state behaves correctly for each transition.
        let states = [
            StreamState::Idle,
            StreamState::ReservedLocal,
            StreamState::ReservedRemote,
            StreamState::Open,
            StreamState::HalfClosedLocal,
            StreamState::HalfClosedRemote,
            StreamState::Closed,
        ];

        for state in &states {
            let mut s = *state;

            // send_headers: Idle + ReservedLocal + Open → OK, others → Err
            let result = s.send_headers(false);
            match *state {
                StreamState::Idle | StreamState::ReservedLocal | StreamState::Open => assert!(result.is_ok(), "{state:?}: send_headers should succeed"),
                _ => assert!(result.is_err(), "{state:?}: send_headers should fail"),
            }

            s = *state;
            // recv_headers: Idle + ReservedRemote + Open + HalfClosedLocal → OK
            let result = s.recv_headers(false);
            match *state {
                StreamState::Idle | StreamState::ReservedRemote | StreamState::Open | StreamState::HalfClosedLocal => assert!(result.is_ok(), "{state:?}: recv_headers should succeed"),
                _ => assert!(result.is_err(), "{state:?}: recv_headers should fail"),
            }

            s = *state;
            // send_data
            let result = s.send_data(false);
            match *state {
                StreamState::Open | StreamState::HalfClosedRemote => assert!(result.is_ok(), "{state:?}: send_data should succeed"),
                _ => assert!(result.is_err(), "{state:?}: send_data should fail"),
            }

            s = *state;
            // recv_data
            let result = s.recv_data(false);
            match *state {
                StreamState::Open | StreamState::HalfClosedLocal => assert!(result.is_ok(), "{state:?}: recv_data should succeed"),
                _ => assert!(result.is_err(), "{state:?}: recv_data should fail"),
            }

            s = *state;
            // send_reset: all but Idle → Closed
            let result = s.send_reset();
            assert_eq!(result.is_ok(), *state != StreamState::Idle, "{state:?}: send_reset");

            s = *state;
            let result = s.recv_reset();
            assert_eq!(result.is_ok(), *state != StreamState::Idle, "{state:?}: recv_reset");
        }
    }
}
