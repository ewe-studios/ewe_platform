//! Tests for `http2::stream` — StreamState FSM (idle→open→half-closed→closed)
//! with HEADERS/DATA/RST_STREAM transition validation (Feature 29).

use foundation_netio::http2::stream::*;

// ── Client-initiated (odd ID) life cycle ───────────────────────────────────

#[test]
fn client_stream_idle_to_open_to_closed() {
    let mut s = StreamState::Idle;
    assert!(s.send_headers(false).is_ok());
    assert_eq!(s, StreamState::Open);
    assert!(s.send_data(true).is_ok());
    assert_eq!(s, StreamState::HalfClosedLocal);
    assert!(s.recv_headers(true).is_ok());
    assert_eq!(s, StreamState::Closed);
}

#[test]
fn server_side_idle_to_open_to_closed() {
    let mut s = StreamState::Idle;
    assert!(s.recv_headers(false).is_ok());
    assert_eq!(s, StreamState::Open);
    assert!(s.send_headers(true).is_ok());
    assert_eq!(s, StreamState::HalfClosedLocal);
    assert!(s.recv_data(true).is_ok());
    assert_eq!(s, StreamState::Closed);
}

// ── Illegal transitions ────────────────────────────────────────────────────

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

// ── RST_STREAM terminates ──────────────────────────────────────────────────

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

// ── Server push ────────────────────────────────────────────────────────────

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

// ── Full matrix ────────────────────────────────────────────────────────────

#[test]
fn all_states_exhaustive() {
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
        let result = s.send_headers(false);
        match *state {
            StreamState::Idle | StreamState::ReservedLocal | StreamState::Open => {
                assert!(result.is_ok(), "{state:?}: send_headers should succeed")
            }
            _ => assert!(result.is_err(), "{state:?}: send_headers should fail"),
        }
        s = *state;
        let result = s.recv_headers(false);
        match *state {
            StreamState::Idle
            | StreamState::ReservedRemote
            | StreamState::Open
            | StreamState::HalfClosedLocal => {
                assert!(result.is_ok(), "{state:?}: recv_headers should succeed")
            }
            _ => assert!(result.is_err(), "{state:?}: recv_headers should fail"),
        }
        s = *state;
        let result = s.send_data(false);
        match *state {
            StreamState::Open | StreamState::HalfClosedRemote => {
                assert!(result.is_ok(), "{state:?}: send_data should succeed")
            }
            _ => assert!(result.is_err(), "{state:?}: send_data should fail"),
        }
        s = *state;
        let result = s.recv_data(false);
        match *state {
            StreamState::Open | StreamState::HalfClosedLocal => {
                assert!(result.is_ok(), "{state:?}: recv_data should succeed")
            }
            _ => assert!(result.is_err(), "{state:?}: recv_data should fail"),
        }
        s = *state;
        assert_eq!(
            s.send_reset().is_ok(),
            *state != StreamState::Idle,
            "{state:?}: send_reset"
        );
        s = *state;
        assert_eq!(
            s.recv_reset().is_ok(),
            *state != StreamState::Idle,
            "{state:?}: recv_reset"
        );
    }
}
