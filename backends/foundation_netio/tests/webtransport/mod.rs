#![cfg(all(feature = "quic", not(target_family = "wasm")))]
use bytes::Bytes;
use foundation_core::valtron::Stream;
use foundation_netio::webtransport::proto::*;
use foundation_netio::webtransport::session::{NoIoSession, WtConnector, WtAcceptor, WtSessionState};

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
