//! WebTransport protocol and session tests (spec-55, F06).

#![cfg(all(feature = "quic", not(target_family = "wasm")))]

use bytes::Bytes;
use foundation_core::valtron::Stream;
use foundation_netio::webtransport::proto::{
    CapsuleType, WtProtocolError, encode_close_session, encode_datagram_capsule, CapsuleDecoder,
};
use foundation_netio::webtransport::session::{WtAcceptor, WtSession, WtSessionState};

// ── One-shot decode_capsule ──

#[test]
fn decode_datagram_capsule() {
    let encoded = encode_datagram_capsule(b"hello");
    let (capsule, consumed) =
        foundation_netio::webtransport::proto::decode_capsule(&encoded).expect("decode");
    assert_eq!(consumed, encoded.len());
    assert!(matches!(capsule, CapsuleType::Datagram(ref d) if d == b"hello"));
}

#[test]
fn decode_close_session_capsule() {
    let encoded = encode_close_session(42, "gone");
    let (capsule, _) =
        foundation_netio::webtransport::proto::decode_capsule(&encoded).expect("decode");
    assert!(
        matches!(capsule, CapsuleType::CloseSession { code: 42, ref reason } if reason == "gone")
    );
}

#[test]
fn decode_truncated_returns_error() {
    assert!(matches!(
        foundation_netio::webtransport::proto::decode_capsule(&[]),
        Err(WtProtocolError::Truncated)
    ));
}

// ── Incremental CapsuleDecoder ──

#[test]
fn incremental_decoder_datagram() {
    let mut dec = CapsuleDecoder::new();
    let encoded = encode_datagram_capsule(b"incremental");
    // Feed first 3 bytes then the rest.
    dec.push(&encoded[..3]);
    assert!(matches!(dec.decode(), Ok(None)));
    dec.push(&encoded[3..]);
    let capsule = dec.decode().expect("decode").expect("capsule");
    assert!(matches!(capsule, CapsuleType::Datagram(ref d) if d == b"incremental"));
    assert!(matches!(dec.decode(), Ok(None)));
    assert!(!dec.has_partial());
}

// ── Session ──

#[test]
fn session_lifecycle() {
    let mut session = WtSession::new(true, true);
    assert!(!session.is_open());
    session.on_connected();
    assert!(session.is_open());
    session.on_capsule(CapsuleType::CloseSession { code: 0, reason: "done".into() });
    assert!(session.is_closed());
    assert_eq!(session.close_info(), Some((0, "done")));
}

#[test]
fn session_datagram_queue_and_drain() {
    let mut session = WtSession::new(true, true);
    session.on_connected();
    session.queue_datagram(Bytes::from("d1")).expect("queue");
    session.queue_datagram(Bytes::from("d2")).expect("queue");
    let drained = session.drain_send_datagrams();
    assert_eq!(drained.len(), 2);
    assert_eq!(&drained[0][..], b"d1");
    assert_eq!(&drained[1][..], b"d2");
}

#[test]
fn session_datagrams_disabled_is_error() {
    let mut session = WtSession::new(true, false);
    session.on_connected();
    assert!(session.queue_datagram(Bytes::from("x")).is_err());
}

#[test]
fn session_drain_capsule() {
    let mut session = WtSession::new(true, true);
    session.on_connected();
    session.on_capsule(CapsuleType::Drain);
    assert_eq!(session.state, WtSessionState::Draining);
}

#[test]
fn acceptor_queue_and_accept() {
    let mut acceptor = WtAcceptor::new();
    assert!(matches!(acceptor.try_accept(), Stream::Pending(())));
    acceptor.queue_session(WtSession::new(false, false));
    assert_eq!(acceptor.pending(), 1);
    assert!(matches!(acceptor.try_accept(), Stream::Next(_)));
}
