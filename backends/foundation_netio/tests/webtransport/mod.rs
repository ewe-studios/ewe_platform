#![cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use foundation_core::valtron::Stream;
use foundation_netio::quic::{QuicBidiStream, QuicConnError, QuicConnection, QuicRecvStream, QuicSendStream, QuicStreamError, StreamId};
use foundation_netio::webtransport::proto::*;
use foundation_netio::webtransport::session::{NoIoSession, WtAcceptor, WtConnector, WtSession, WtSessionState};

#[test] fn decode_datagram_capsule() { let e = encode_datagram_capsule(b"h"); let (c,_) = decode_capsule(&e).unwrap(); assert!(matches!(c, CapsuleType::Datagram(ref d) if d == b"h")); }
#[test] fn decode_close_session() { let e = encode_close_session(42,"g"); let (c,_) = decode_capsule(&e).unwrap(); assert!(matches!(c, CapsuleType::CloseSession{code:42,..})); }
#[test] fn decode_truncated() { assert!(matches!(decode_capsule(&[]), Err(WtProtocolError::Truncated))); }

#[test] fn incremental_decoder() {
    let mut d = CapsuleDecoder::new();
    let e = encode_datagram_capsule(b"incremental");
    d.push(&e[..3]); assert!(matches!(d.decode(), Ok(None)));
    d.push(&e[3..]); let c = d.decode().unwrap().unwrap();
    assert!(matches!(c, CapsuleType::Datagram(ref b) if b == b"incremental"));
    assert!(matches!(d.decode(), Ok(None))); assert!(!d.has_partial());
}

#[test] fn session_lifecycle() {
    let mut s = NoIoSession::new(true,true); assert!(!s.is_open());
    s.on_connected(); assert!(s.is_open());
    s.on_capsule(CapsuleType::CloseSession{code:0,reason:"d".into()}); assert!(s.is_closed());
}
#[test] fn session_datagrams() {
    let mut s = NoIoSession::new(true,true); s.on_connected();
    s.queue_datagram(Bytes::from("d1")).unwrap();
    s.queue_datagram(Bytes::from("d2")).unwrap();
    assert_eq!(s.drain_send_datagrams().len(),2);
}
#[test] fn session_datagrams_disabled() {
    let mut s = NoIoSession::new(true,false); s.on_connected();
    assert!(s.queue_datagram(Bytes::from("x")).is_err());
}
#[test] fn session_drain() {
    let mut s = NoIoSession::new(true,true); s.on_connected();
    s.on_capsule(CapsuleType::Drain); assert_eq!(s.state, WtSessionState::Draining);
}

#[test] fn connector_headers() {
    let h = WtConnector::build_connect_headers("ex.com","/wt");
    assert!(h.iter().any(|(k,v)| k==b":method" && v==b"CONNECT"));
    assert!(h.iter().any(|(k,v)| k==b":protocol" && v==b"webtransport"));
}
#[test] fn response_headers() {
    let h: Vec<(Vec<u8>,Vec<u8>)> = vec![(b":status".into(),b"200".into()),(b"sec-webtransport-http3-draft".into(),b"draft-07".into())];
    let has200 = h.iter().any(|(k,v)| k==b":status" && v==b"200");
    assert!(has200);
}

// ── Mock QuicConnection for end-to-end WtSession<C> tests ──────────────
// Proves WtSession<C: QuicConnection> compiles and works with a concrete
// stream type beyond NoIoSession. The mock has in-memory datagram queues
// and a close latch — enough to exercise the full session lifecycle.


/// Shared state for the mock: datagram queues and a closed flag.
struct MockShared {
    send_dgrams: VecDeque<Bytes>,
    recv_dgrams: VecDeque<Bytes>,
    closed: bool,
}

/// A mock QUIC connection. All stream opens return `Pending` (no mock streams);
/// datagram send/recv operate through the in-memory queues.
struct MockQuicConn {
    shared: Arc<Mutex<MockShared>>,
}

impl MockQuicConn {
    fn new() -> Self {
        Self { shared: Arc::new(Mutex::new(MockShared {
            send_dgrams: VecDeque::new(),
            recv_dgrams: VecDeque::new(),
            closed: false,
        }))}
    }
    /// Feed a datagram into the mock's receive queue (simulates network arrival).
    fn inject_datagram(&self, data: Bytes) {
        self.shared.lock().unwrap().recv_dgrams.push_back(data);
    }
    /// Drain outbound datagrams (simulates network delivery to peer).
    fn drain_sent_datagrams(&self) -> Vec<Bytes> {
        self.shared.lock().unwrap().send_dgrams.drain(..).collect()
    }
}

// Minimal mock stream types — needed for associated types, never instantiated.
struct MockSendStream;
struct MockRecvStream;
struct MockBidiStream;

impl QuicSendStream for MockSendStream {
    fn send(&mut self, _buf: &mut impl bytes::Buf) -> Stream<Result<usize, QuicStreamError>, ()> {
        Stream::Pending(())
    }
    fn finish(&mut self) -> Stream<Result<(), QuicStreamError>, ()> { Stream::Pending(()) }
    fn reset(&mut self, _code: u64) {}
    fn id(&self) -> StreamId { StreamId::new(0) }
}

impl QuicRecvStream for MockRecvStream {
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()> { Stream::Pending(()) }
    fn stop_sending(&mut self, _code: u64) {}
    fn id(&self) -> StreamId { StreamId::new(0) }
}

impl QuicSendStream for MockBidiStream {
    fn send(&mut self, _buf: &mut impl bytes::Buf) -> Stream<Result<usize, QuicStreamError>, ()> {
        Stream::Pending(())
    }
    fn finish(&mut self) -> Stream<Result<(), QuicStreamError>, ()> { Stream::Pending(()) }
    fn reset(&mut self, _code: u64) {}
    fn id(&self) -> StreamId { StreamId::new(0) }
}
impl QuicRecvStream for MockBidiStream {
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()> { Stream::Pending(()) }
    fn stop_sending(&mut self, _code: u64) {}
    fn id(&self) -> StreamId { StreamId::new(0) }
}
impl QuicBidiStream for MockBidiStream {
    fn split(self) -> (impl QuicSendStream, impl QuicRecvStream) where Self: Sized {
        (MockSendStream, MockRecvStream)
    }
}

impl QuicConnection for MockQuicConn {
    type RecvStream = MockRecvStream;
    type SendStream = MockSendStream;
    type BidiStream = MockBidiStream;

    fn accept_recv(&mut self) -> Stream<Result<Self::RecvStream, QuicConnError>, ()> { Stream::Pending(()) }
    fn accept_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicConnError>, ()> { Stream::Pending(()) }
    fn open_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicStreamError>, ()> { Stream::Pending(()) }
    fn open_send(&mut self) -> Stream<Result<Self::SendStream, QuicStreamError>, ()> { Stream::Pending(()) }

    fn close(&mut self, _code: u64, _reason: &[u8]) {
        self.shared.lock().unwrap().closed = true;
    }

    fn send_datagram(&mut self, data: &[u8]) -> Stream<Result<(), QuicStreamError>, ()> {
        let mut s = self.shared.lock().unwrap();
        if s.closed { return Stream::Next(Err(QuicStreamError::ConnClosed(QuicConnError::ApplicationClose { code: 0 }))); }
        s.send_dgrams.push_back(Bytes::copy_from_slice(data));
        Stream::Next(Ok(()))
    }

    fn recv_datagram(&mut self) -> Stream<Result<Option<Bytes>, QuicConnError>, ()> {
        let mut s = self.shared.lock().unwrap();
        if s.closed { return Stream::Next(Err(QuicConnError::ApplicationClose { code: 0 })); }
        if let Some(d) = s.recv_dgrams.pop_front() {
            Stream::Next(Ok(Some(d)))
        } else {
            Stream::Pending(())
        }
    }

    fn max_datagram_size(&self) -> Option<usize> { Some(1200) }
}

// ── End-to-end WtSession<MockQuicConn> tests ───────────────────────────
// Exercises the full session lifecycle with a concrete QuicConnection impl.

#[test] fn wt_session_with_mock_quic_datagram_roundtrip() {
    let mqc = MockQuicConn::new();
    let mut session: WtSession<MockQuicConn> = WtSession::new(mqc, true);
    assert!(!session.is_open());
    session.on_connected();
    assert!(session.is_open());

    // Queue outbound datagrams and flush — they land in the mock's send queue.
    session.queue_datagram(Bytes::from("hello")).unwrap();
    session.queue_datagram(Bytes::from("world")).unwrap();
    assert!(matches!(session.flush_datagrams(), Stream::Next(Ok(()))));
    let sent = session.conn.drain_sent_datagrams();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], Bytes::from("hello"));
    assert_eq!(sent[1], Bytes::from("world"));

    // Inject inbound datagrams — pump_recv_datagrams picks them up.
    session.conn.inject_datagram(Bytes::from("pong"));
    session.pump_recv_datagrams();
    match session.try_recv_datagram() {
        Stream::Next(Ok(Some(d))) => assert_eq!(d, Bytes::from("pong")),
        other => panic!("expected datagram, got {other:?}"),
    }
    // No more datagrams → Pending.
    assert!(matches!(session.try_recv_datagram(), Stream::Pending(())));
}

#[test] fn wt_session_close_notifies_datagrams() {
    let mqc = MockQuicConn::new();
    let mut session: WtSession<MockQuicConn> = WtSession::new(mqc, true);
    session.on_connected();

    session.on_capsule(CapsuleType::CloseSession { code: 42, reason: "bye".into() });
    assert!(session.is_closed());
    assert_eq!(session.close_info(), Some((42, "bye")));

    // Datagram send after close → error.
    assert!(session.queue_datagram(Bytes::from("x")).is_err());
}

#[test] fn wt_session_datagrams_disabled() {
    let mqc = MockQuicConn::new();
    let mut session: WtSession<MockQuicConn> = WtSession::new(mqc, false);
    assert!(!session.datagrams_enabled());
    assert!(session.queue_datagram(Bytes::from("x")).is_err());
}

#[test] fn wt_session_drain_capsule() {
    let mqc = MockQuicConn::new();
    let mut session: WtSession<MockQuicConn> = WtSession::new(mqc, true);
    session.on_connected();
    assert!(session.on_capsule(CapsuleType::Drain));
    assert_eq!(session.state, WtSessionState::Draining);
}

#[test] fn wt_acceptor_queue_and_accept() {
    let mqc = MockQuicConn::new();
    let mut acceptor: WtAcceptor<MockQuicConn> = WtAcceptor::new();
    assert_eq!(acceptor.pending(), 0);

    let session = WtSession::new(mqc, true);
    acceptor.queue_session(session);
    assert_eq!(acceptor.pending(), 1);

    match acceptor.try_accept() {
        Stream::Next(s) => assert!(!s.is_open()),
        _ => panic!("expected session"),
    }
    assert_eq!(acceptor.pending(), 0);
    assert!(matches!(acceptor.try_accept(), Stream::Pending(())));
}
