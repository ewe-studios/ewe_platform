//! HTTP/3 → Simple types mapping (RFC 9114 §4).
//!
//! WHY: Decision 01 promises a handler cannot tell which transport carried its
//! request. That promise only holds if HTTP/3's pseudo-headers fold into exactly
//! the same `SimpleIncomingRequestHeader` HTTP/2 produces — and if the requests
//! HTTP/3 declares malformed never reach a handler at all.
//!
//! WHAT: the pseudo-header contract, the §4.2 forbidden fields, §4.1.1's
//! lowercase rule, and the `:status` pseudo-header on the way out.

use std::sync::Arc;

use bytes::Bytes;

use foundation_netio::http3::types::{request_from_fields, response_to_fields, MalformedRequest};
use foundation_netio::netcap::context::ConnectionContext;
use foundation_netio::simple_http::shared::{
    SimpleHeader, SimpleMethod, SimpleOutgoingResponse, Status,
};

fn field(name: &str, value: &str) -> (Bytes, Bytes) {
    (
        Bytes::copy_from_slice(name.as_bytes()),
        Bytes::copy_from_slice(value.as_bytes()),
    )
}

/// A minimal well-formed request, plus whatever extra fields the caller wants.
fn request(extra: &[(&str, &str)]) -> Vec<(Bytes, Bytes)> {
    let mut fields = vec![
        field(":method", "POST"),
        field(":scheme", "https"),
        field(":authority", "example.com"),
        field(":path", "/svc/Method"),
    ];
    fields.extend(extra.iter().map(|(n, v)| field(n, v)));
    fields
}

fn ctx() -> Arc<ConnectionContext> {
    Arc::new(ConnectionContext::default())
}

#[test]
fn a_well_formed_request_maps_onto_the_shared_simple_types() {
    let fields = request(&[("content-type", "application/grpc"), ("te", "trailers")]);
    let header = request_from_fields(&fields, ctx()).expect("well-formed request");

    assert_eq!(header.method, SimpleMethod::POST);
    assert_eq!(header.scheme, "https");
    assert_eq!(header.authority, "example.com");
    assert_eq!(header.path, "/svc/Method");

    // The routing key is origin-form: routers match on paths, not absolute URLs.
    assert_eq!(header.url.to_string(), "/svc/Method");

    let content_type = header
        .headers
        .get(&SimpleHeader::from("content-type".to_string()))
        .expect("content-type survives");
    assert_eq!(content_type, &vec!["application/grpc".to_string()]);

    // Pseudo-headers must not leak into the regular header map.
    assert!(
        header
            .headers
            .get(&SimpleHeader::from(":method".to_string()))
            .is_none(),
        "pseudo-headers are not regular fields"
    );
}

#[test]
fn every_method_survives_the_mapping() {
    // Regression: the shared `header_from_hpack` used to map anything that was not
    // GET onto POST, so PUT/DELETE/PATCH over HTTP/2 and HTTP/3 were silently
    // rewritten. gRPC only ever sends POST, which is why it went unnoticed.
    for (wire, expected) in [
        ("GET", SimpleMethod::GET),
        ("POST", SimpleMethod::POST),
        ("PUT", SimpleMethod::PUT),
        ("DELETE", SimpleMethod::DELETE),
        ("PATCH", SimpleMethod::PATCH),
        ("HEAD", SimpleMethod::HEAD),
        ("OPTIONS", SimpleMethod::OPTIONS),
    ] {
        let fields = vec![
            field(":method", wire),
            field(":scheme", "https"),
            field(":path", "/"),
        ];
        let header = request_from_fields(&fields, ctx()).expect("well-formed");
        assert_eq!(header.method, expected, "{wire} must not be rewritten");
    }
}

#[test]
fn a_request_without_authority_is_still_well_formed() {
    // §4.3.1: `:authority` is optional — a `Host` field may stand in.
    let fields = vec![
        field(":method", "GET"),
        field(":scheme", "https"),
        field(":path", "/health"),
    ];
    let header = request_from_fields(&fields, ctx()).expect("origin-form request");
    assert_eq!(header.authority, "");
    assert_eq!(header.path, "/health");
}

#[test]
fn missing_mandatory_pseudo_headers_are_rejected() {
    for (missing, fields) in [
        (
            ":method",
            vec![field(":scheme", "https"), field(":path", "/")],
        ),
        (
            ":scheme",
            vec![field(":method", "GET"), field(":path", "/")],
        ),
        (
            ":path",
            vec![field(":method", "GET"), field(":scheme", "https")],
        ),
    ] {
        match request_from_fields(&fields, ctx()) {
            Err(MalformedRequest::MissingPseudoHeader(p)) => assert_eq!(p, missing),
            other => panic!("expected {missing} to be required, got {other:?}"),
        }
    }
}

#[test]
fn connection_specific_fields_are_rejected() {
    // RFC 9114 §4.2. QUIC has no hop-by-hop framing for these to describe, and a
    // proxy that forwarded them would be smuggling.
    for forbidden in [
        "connection",
        "keep-alive",
        "proxy-connection",
        "transfer-encoding",
        "upgrade",
    ] {
        let fields = request(&[(forbidden, "whatever")]);
        match request_from_fields(&fields, ctx()) {
            Err(MalformedRequest::ConnectionSpecificField(name)) => assert_eq!(name, forbidden),
            other => panic!("{forbidden} must be rejected, got {other:?}"),
        }
    }
}

#[test]
fn te_is_allowed_only_with_the_value_trailers() {
    assert!(request_from_fields(&request(&[("te", "trailers")]), ctx()).is_ok());

    match request_from_fields(&request(&[("te", "gzip")]), ctx()) {
        Err(MalformedRequest::BadTe(v)) => assert_eq!(v, "gzip"),
        other => panic!("TE: gzip must be rejected, got {other:?}"),
    }
}

#[test]
fn uppercase_field_names_are_rejected() {
    // §4.1.1: field names must be lowercase on the wire.
    match request_from_fields(&request(&[("Content-Type", "text/plain")]), ctx()) {
        Err(MalformedRequest::UppercaseFieldName(n)) => assert_eq!(n, "Content-Type"),
        other => panic!("an uppercase field name must be rejected, got {other:?}"),
    }
}

#[test]
fn a_pseudo_header_after_a_regular_field_is_rejected() {
    // §4.3: all pseudo-headers precede the regular fields.
    let fields = vec![
        field(":method", "GET"),
        field(":scheme", "https"),
        field("accept", "*/*"),
        field(":path", "/late"),
    ];
    match request_from_fields(&fields, ctx()) {
        Err(MalformedRequest::PseudoHeaderAfterField(p)) => assert_eq!(p, ":path"),
        other => panic!("a late pseudo-header must be rejected, got {other:?}"),
    }
}

#[test]
fn a_response_carries_a_bare_status_pseudo_header_first() {
    let mut response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .build()
        .expect("response");
    response.headers.insert(
        SimpleHeader::from("content-type".to_string()),
        vec!["application/grpc".into()],
    );

    let fields = response_to_fields(&response);

    assert_eq!(fields[0].0, Bytes::from_static(b":status"));
    assert_eq!(
        fields[0].1,
        Bytes::from_static(b"200"),
        "`:status` is the bare code — `Status`'s Display renders \"200 OK\", which \
         would be a malformed pseudo-header"
    );

    assert!(
        fields
            .iter()
            .any(|(n, v)| n == "content-type" && v == "application/grpc"),
        "regular fields follow: {fields:?}"
    );
}

#[test]
fn a_response_lowercases_field_names() {
    let mut response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .build()
        .expect("response");
    response.headers.insert(
        SimpleHeader::from("X-Custom-Header".to_string()),
        vec!["v".into()],
    );

    let fields = response_to_fields(&response);
    assert!(
        fields.iter().any(|(n, _)| n == "x-custom-header"),
        "§4.1.1 requires lowercase names on the wire: {fields:?}"
    );
    assert!(
        !fields.iter().any(|(n, _)| n == "X-Custom-Header"),
        "the uppercase form must not survive"
    );
}

#[test]
fn a_response_drops_connection_specific_fields_rather_than_sending_them() {
    // A handler that sets `Connection: keep-alive` out of HTTP/1.1 habit should not
    // make our response malformed, and should not take the connection down.
    let mut response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .build()
        .expect("response");
    response.headers.insert(
        SimpleHeader::from("Connection".to_string()),
        vec!["keep-alive".into()],
    );
    response.headers.insert(
        SimpleHeader::from("transfer-encoding".to_string()),
        vec!["chunked".into()],
    );

    let fields = response_to_fields(&response);
    assert!(
        !fields
            .iter()
            .any(|(n, _)| n == "connection" || n == "transfer-encoding"),
        "RFC 9114 §4.2 forbids these on the wire: {fields:?}"
    );
    assert_eq!(fields.len(), 1, "only :status survives: {fields:?}");
}

#[test]
fn a_request_round_trips_through_qpack() {
    // The whole path a HEADERS frame takes: encode → wire → decode → Simple types.
    use foundation_netio::http3::{decode_field_section, encode_field_section};

    let original = request(&[("content-type", "application/grpc"), ("te", "trailers")]);
    let pairs: Vec<(&[u8], &[u8])> = original.iter().map(|(n, v)| (&n[..], &v[..])).collect();

    let wire = encode_field_section(&pairs);
    let decoded = decode_field_section(&wire, 1 << 20).expect("decode");

    let header = request_from_fields(&decoded, ctx()).expect("well-formed after round-trip");
    assert_eq!(header.method, SimpleMethod::POST);
    assert_eq!(header.path, "/svc/Method");
    assert_eq!(header.authority, "example.com");
}
