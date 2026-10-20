//! Forwarding header logic: hop-by-hop stripping and upgrade detection.

use std::collections::BTreeMap;

use foundation_netio::shared::http::{
    SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use foundation_proxy::forward::{
    forward_request_headers, is_streaming_request, is_upgrade_request, strip_hop_by_hop,
    strip_hop_by_hop_request,
};

/// WHY: Hop-by-hop headers describe a single TCP hop and must never be
/// forwarded; anything named in `Connection` is hop-by-hop too (RFC 7230 §6.1).
/// WHAT: Fixed hop-by-hop headers and a `Connection`-named custom header are
/// removed; end-to-end headers survive.
#[test]
fn test_strip_removes_hop_by_hop_and_connection_listed() {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::CONNECTION, vec!["close, x-custom".to_string()]);
    headers.insert(SimpleHeader::custom("x-custom"), vec!["secret".to_string()]);
    headers.insert(SimpleHeader::TRANSFER_ENCODING, vec!["chunked".to_string()]);
    headers.insert(SimpleHeader::KEEP_ALIVE, vec!["timeout=5".to_string()]);
    headers.insert(SimpleHeader::CONTENT_TYPE, vec!["text/plain".to_string()]);
    headers.insert(SimpleHeader::HOST, vec!["app.example.com".to_string()]);

    let stripped = strip_hop_by_hop(&headers);

    assert!(!stripped.contains_key(&SimpleHeader::CONNECTION), "Connection is hop-by-hop");
    assert!(
        !stripped.contains_key(&SimpleHeader::custom("x-custom")),
        "Connection-listed header must be dropped"
    );
    assert!(!stripped.contains_key(&SimpleHeader::TRANSFER_ENCODING), "Transfer-Encoding is hop-by-hop");
    assert!(!stripped.contains_key(&SimpleHeader::KEEP_ALIVE), "Keep-Alive is hop-by-hop");
    assert!(stripped.contains_key(&SimpleHeader::CONTENT_TYPE), "Content-Type is end-to-end");
    assert!(stripped.contains_key(&SimpleHeader::HOST), "Host is preserved");
}

/// WHY: WebSocket relaying triggers only on a genuine upgrade request.
/// WHAT: `Connection: upgrade` + an `Upgrade` header is an upgrade; either alone
/// is not.
#[test]
fn test_upgrade_detection() {
    let ws = SimpleIncomingRequest::builder()
        .with_plain_url("/ws")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::CONNECTION, "Upgrade")
        .add_header(SimpleHeader::UPGRADE, "websocket")
        .build()
        .expect("build ws request");
    assert!(is_upgrade_request(&ws), "Connection: upgrade + Upgrade header is an upgrade");

    let plain = SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::CONNECTION, "keep-alive")
        .build()
        .expect("build plain request");
    assert!(!is_upgrade_request(&plain), "no Upgrade header → not an upgrade");

    let missing_conn = SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::UPGRADE, "websocket")
        .build()
        .expect("build request");
    assert!(
        !is_upgrade_request(&missing_conn),
        "Upgrade header without Connection: upgrade → not an upgrade"
    );
}

/// WHY: gRPC mandates `te: trailers` end-to-end (RFC 9113 §8.2.2); a proxy that
/// strips it as hop-by-hop silently breaks every gRPC call. The request path must
/// preserve it, along with the `trailer`/`trailers` announcement names, while
/// still stripping the rest of the hop-by-hop set.
/// WHAT: `strip_hop_by_hop_request` keeps `te: trailers`, `trailer`, `trailers`;
/// it still removes `Connection`, `Transfer-Encoding`, `Keep-Alive`, and any
/// `Connection`-listed custom header.
#[test]
fn test_request_strip_preserves_grpc_trailer_headers() {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::TE, vec!["trailers".to_string()]);
    headers.insert(SimpleHeader::TRAILER, vec!["grpc-status".to_string()]);
    headers.insert(SimpleHeader::custom("trailers"), vec!["grpc-message".to_string()]);
    headers.insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);
    headers.insert(SimpleHeader::TRANSFER_ENCODING, vec!["chunked".to_string()]);
    headers.insert(SimpleHeader::KEEP_ALIVE, vec!["timeout=5".to_string()]);
    headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/grpc+json".to_string()]);

    let stripped = strip_hop_by_hop_request(&headers);

    assert!(stripped.contains_key(&SimpleHeader::TE), "te: trailers must survive");
    assert!(stripped.contains_key(&SimpleHeader::TRAILER), "trailer must survive");
    assert!(
        stripped.contains_key(&SimpleHeader::custom("trailers")),
        "trailers must survive"
    );
    assert!(!stripped.contains_key(&SimpleHeader::CONNECTION), "Connection is hop-by-hop");
    assert!(!stripped.contains_key(&SimpleHeader::TRANSFER_ENCODING), "Transfer-Encoding is hop-by-hop");
    assert!(!stripped.contains_key(&SimpleHeader::KEEP_ALIVE), "Keep-Alive is hop-by-hop");
    assert!(stripped.contains_key(&SimpleHeader::CONTENT_TYPE), "Content-Type is end-to-end");
}

/// WHY: only the gRPC-mandated `trailers` token may be forwarded on `te`; any
/// other `te` value is hop-by-hop and must be stripped.
/// WHAT: `strip_hop_by_hop_request` drops `te: gzip`.
#[test]
fn test_request_strip_drops_non_trailers_te() {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::TE, vec!["gzip".to_string()]);

    let stripped = strip_hop_by_hop_request(&headers);

    assert!(!stripped.contains_key(&SimpleHeader::TE), "te: gzip must be stripped");
}

fn headers_with_content_type(ct: &str) -> SimpleHeaders {
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::CONTENT_TYPE, vec![ct.to_string()]);
    headers.insert(SimpleHeader::HOST, vec!["app.example.com".to_string()]);
    headers
}

/// WHY: RPC streaming responses are always length- or chunk-delimited, so the
/// upstream `Connection: close` the proxy adds for plain requests is unnecessary
/// — and hostile to long-lived streams (it forces the backend to drop the
/// connection after each stream, defeating reuse). The proxy must recognise RPC
/// streaming media types and relax the close policy for them.
/// WHAT: `is_streaming_request` is true for gRPC / Connect-streaming / gRPC-Web
/// content types and false for plain media types.
#[test]
fn test_is_streaming_request() {
    for ct in [
        "application/grpc",
        "application/grpc+proto",
        "application/grpc+json",
        "application/connect+proto",
        "application/connect+json",
        "application/grpc-web",
        "application/grpc-web+proto",
        "application/grpc-web-text",
    ] {
        assert!(is_streaming_request(&headers_with_content_type(ct)), "{ct} is a streaming RPC type");
    }
    for ct in ["application/json", "application/proto", "text/plain", "application/octet-stream"] {
        assert!(!is_streaming_request(&headers_with_content_type(ct)), "{ct} is not streaming RPC");
    }
}

/// WHY: `Connection: close` must stay for plain requests (EOF-delimited bodies
/// need it), but must be omitted for streaming RPC so long-lived streams are not
/// torn down and upstream connections can be reused.
/// WHAT: `forward_request_headers` sets `Connection: close` for `text/plain` and
/// omits it for gRPC / Connect-streaming / gRPC-Web.
#[test]
fn test_forward_headers_relax_connection_close_for_streams() {
    let plain = forward_request_headers(&headers_with_content_type("text/plain"), "1.2.3.4", "http");
    assert!(
        plain.contains_key(&SimpleHeader::CONNECTION),
        "plain requests still get Connection: close"
    );

    for ct in [
        "application/grpc",
        "application/grpc+proto",
        "application/connect+proto",
        "application/grpc-web+proto",
    ] {
        let out = forward_request_headers(&headers_with_content_type(ct), "1.2.3.4", "http");
        assert!(
            !out.contains_key(&SimpleHeader::CONNECTION),
            "{ct} must not get Connection: close"
        );
    }
}
