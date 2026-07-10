//! HTTP/3 transport capabilities and the capability matrix (F35).
//!
//! WHY: Decision 11 turns "any protocol on any transport" into an enforced
//! predicate: a protocol declares what a call needs, a transport declares what it
//! offers, and `check_compatible` rejects mismatches uniformly. Adding a transport
//! means adding a row to that matrix — and the row has to be *true*, or the check
//! passes calls the transport cannot actually serve.
//!
//! WHAT: what `H3Transport` claims, and that gRPC (which needs HTTP/2+ and
//! trailers) is compatible with it while HTTP/1.1-only calls are not confused with
//! it.
//!
//! HOW: pure capability checks — no sockets. The wire behaviour those claims rest
//! on is proved in `foundation_netio`'s `tests/http3/connection_tests.rs`, which
//! round-trips a real request and response over live QUIC.

#![cfg(feature = "h3")]

use foundation_netio::simple_http::shared::Proto;

use foundation_connectrpc::context::StreamType;
use foundation_connectrpc::transport::capabilities::{
    check_compatible, requirements, ProtocolKind,
};
use foundation_connectrpc::transport::h3::H3Transport;
use foundation_connectrpc::transport::Transport;

/// A transport instance. The client config is never used: no call is opened.
fn transport() -> H3Transport {
    let cert = include_bytes!("../../foundation_netio/tests/fixtures/quic_ca.pem");
    let cfg = foundation_netio::quic::client_config_trusting_pem(cert).expect("client config");
    H3Transport::new(cfg)
}

#[test]
fn http3_advertises_what_quic_actually_provides() {
    let caps = transport().capabilities();

    assert!(caps.request_streaming, "HTTP/3 can keep sending DATA frames");
    assert!(
        caps.full_duplex,
        "QUIC gives each direction its own flow-control window, and the pump \
         advances both every poll"
    );
    assert!(
        caps.h2_trailers,
        "HTTP/3's trailers are a second HEADERS frame — the capability gRPC needs"
    );
    assert_eq!(caps.http_versions, &[Proto::HTTP30]);
}

#[test]
fn grpc_is_compatible_with_http3_in_every_stream_mode() {
    // gRPC requires HTTP/2 *or later* and trailing HEADERS. HTTP/3 has both, so
    // every mode must pass — including bidi, which needs full duplex.
    let caps = transport().capabilities();

    for mode in [
        StreamType::Unary,
        StreamType::ServerStream,
        StreamType::ClientStream,
        StreamType::BidiStream,
    ] {
        let req = requirements(ProtocolKind::Grpc, mode);
        check_compatible(&req, &caps)
            .unwrap_or_else(|e| panic!("gRPC {mode:?} must run over HTTP/3: {e:?}"));
    }
}

#[test]
fn connect_and_grpc_web_are_compatible_with_http3() {
    let caps = transport().capabilities();

    for protocol in [ProtocolKind::Connect, ProtocolKind::GrpcWeb] {
        for mode in [
            StreamType::Unary,
            StreamType::ServerStream,
            StreamType::ClientStream,
            StreamType::BidiStream,
        ] {
            let req = requirements(protocol, mode);
            check_compatible(&req, &caps)
                .unwrap_or_else(|e| panic!("{protocol:?} {mode:?} must run over HTTP/3: {e:?}"));
        }
    }
}

#[test]
fn the_min_version_check_accepts_http3_for_a_protocol_that_demands_http2() {
    // `Proto` orders HTTP10 < HTTP11 < HTTP20 < HTTP30, so a transport offering
    // only HTTP/3 satisfies gRPC's `min_http_version = HTTP20`. If that ordering
    // ever changed, gRPC over HTTP/3 would start being rejected as "too old" —
    // which is why this asserts the ordering directly rather than trusting it.
    assert!(Proto::HTTP30 > Proto::HTTP20);
    assert!(Proto::HTTP20 > Proto::HTTP11);

    let req = requirements(ProtocolKind::Grpc, StreamType::Unary);
    assert_eq!(req.min_http_version, Proto::HTTP20);
    assert!(req.h2_trailers, "gRPC's status lives in trailers");

    check_compatible(&req, &transport().capabilities()).expect("HTTP/3 satisfies HTTP/2+");
}

#[test]
fn a_transport_without_trailers_still_rejects_grpc() {
    // Guard against the matrix becoming permissive: the check must still bite.
    let mut caps = transport().capabilities();
    caps.h2_trailers = false;

    let req = requirements(ProtocolKind::Grpc, StreamType::Unary);
    let err = check_compatible(&req, &caps)
        .expect_err("gRPC without trailing HEADERS must be rejected");
    assert!(
        format!("{err:?}").contains("trailing HEADERS"),
        "the error must name the missing ability: {err:?}"
    );
}

#[test]
fn a_transport_without_full_duplex_still_rejects_bidi() {
    let mut caps = transport().capabilities();
    caps.full_duplex = false;

    let req = requirements(ProtocolKind::Connect, StreamType::BidiStream);
    let err = check_compatible(&req, &caps).expect_err("bidi needs full duplex");
    assert!(
        format!("{err:?}").contains("full-duplex"),
        "the error must name the missing ability: {err:?}"
    );
}
