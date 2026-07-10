//! Forwarding header logic: hop-by-hop stripping and upgrade detection.

use std::collections::BTreeMap;

use foundation_netio::simple_http::shared::{
    SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use foundation_proxy::forward::{is_upgrade_request, strip_hop_by_hop};

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
