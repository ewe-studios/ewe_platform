#![allow(dead_code)]
// Unit tests for `impls` simple_incoming behavior moved into canonical units tree.
//
// Non-destructive copies of the original in-crate tests focused on the
// `SimpleIncomingRequest` conversion/representation. These are intended to be
// fast, deterministic unit tests that exercise the public builder conversion
// surface exposed by `PreparedRequest::into_simple_incoming_request()`.

use foundation_core::wire::simple_http::client::{ClientRequestBuilder, StaticSocketAddr};
use foundation_core::wire::simple_http::{
    Http11, Proto, RenderHttp, SimpleHeader, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use std::net::SocketAddr as StdSocketAddr;

/// Verify that a `PreparedRequest` built from the public builder converts into
/// a `SimpleIncomingRequest` with the expected method and protocol.
#[test]
fn should_convert_prepared_request_to_simple_incoming() {
    let resolver = StaticSocketAddr::new(StdSocketAddr::from(([127, 0, 0, 1], 80)));
    let prepared = ClientRequestBuilder::get(resolver, "http://example.com/path")
        .unwrap()
        .build()
        .unwrap();

    let simple_request = prepared.into_simple_incoming_request().unwrap();

    // Basic expectations: method and protocol set correctly
    assert_eq!(simple_request.method, SimpleMethod::GET);
    assert_eq!(simple_request.proto, Proto::HTTP11);
}

/// Verify that headers set on the builder are preserved when converting to
/// `SimpleIncomingRequest`.
#[test]
fn should_preserve_headers_on_simple_incoming_request() {
    let resolver = StaticSocketAddr::new(StdSocketAddr::from(([127, 0, 0, 1], 80)));
    let builder = ClientRequestBuilder::post(resolver, "http://example.com/upload")
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json")
        .body_text("{\"ok\":true}");

    let prepared = builder.build().unwrap();
    let simple_request = prepared.into_simple_incoming_request().unwrap();

    // Check that the rendered headers contain the expected entries.
    // `SimpleIncomingRequest` exposes a headers container; assert common headers exist.
    assert!(simple_request
        .headers
        .contains_key(&SimpleHeader::CONTENT_TYPE));
    assert!(simple_request
        .headers
        .contains_key(&SimpleHeader::CONTENT_LENGTH));
}

/// Verify that query strings from the prepared URL are preserved in the
/// `SimpleIncomingRequest` request URL rendering.
#[test]
fn should_preserve_query_in_request_url() {
    let resolver = StaticSocketAddr::new(StdSocketAddr::from(([127, 0, 0, 1], 80)));
    let prepared = ClientRequestBuilder::get(resolver, "http://example.com/search?q=test&limit=10")
        .unwrap()
        .build()
        .unwrap();

    let simple_request = prepared.into_simple_incoming_request().unwrap();

    // The `request_url` field on `SimpleIncomingRequest` contains the rendered URL.
    // Ensure the query portion is preserved.
    let rendered = &simple_request.request_url.url;
    assert!(rendered.contains("?q=test&limit=10") || rendered.contains("?limit=10&q=test"));
}

#[test]
fn should_convert_to_get_request_header_without_body_for_descriptor() {
    let prepared = SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .add_header(SimpleHeader::HOST, "localhost:8000")
        .add_header(SimpleHeader::Custom("X-VILLA".into()), "YES")
        .with_body_string("Hello")
        .build()
        .unwrap();

    let request = Http11::request_descriptor(prepared.descriptor());

    assert_eq!(
        request.http_render_string().unwrap(),
        "GET / HTTP/1.1\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\nX-VILLA: YES\r\n\r\n"
    );
}

#[test]
fn should_convert_to_get_request_with_custom_header() {
    let request = Http11::request(
        SimpleIncomingRequest::builder()
            .with_plain_url("/")
            .with_method(SimpleMethod::GET)
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .add_header(SimpleHeader::HOST, "localhost:8000")
            .add_header(SimpleHeader::Custom("X-VILLA".into()), "YES")
            .with_body_string("Hello")
            .build()
            .unwrap(),
    );

    assert_eq!(
        request.http_render_string().unwrap(),
        "GET / HTTP/1.1\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\nX-VILLA: YES\r\n\r\nHello"
    );
}

#[test]
fn should_convert_to_get_request() {
    let request = Http11::request(
        SimpleIncomingRequest::builder()
            .with_plain_url("/")
            .with_method(SimpleMethod::GET)
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .add_header(SimpleHeader::HOST, "localhost:8000")
            .with_body_string("Hello")
            .build()
            .unwrap(),
    );

    assert_eq!(
        request.http_render_string().unwrap(),
        "GET / HTTP/1.1\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\nHello"
    );
}

#[test]
fn should_convert_to_get_response() {
    let request = Http11::response(
        SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .add_header(SimpleHeader::HOST, "localhost:8000")
            .with_body_string("Hello")
            .build()
            .unwrap(),
    );

    assert_eq!(
        request.http_render_string().unwrap(),
        "HTTP/1.1 200 Ok\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\nHello"
    );
}

#[test]
fn should_convert_to_get_response_with_custom_status() {
    let request = Http11::response(
        SimpleOutgoingResponse::builder()
            .with_status(Status::Numbered(666, "Custom status".into()))
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .add_header(SimpleHeader::HOST, "localhost:8000")
            .with_body_string("Hello")
            .build()
            .unwrap(),
    );

    assert_eq!(
        request.http_render_string().unwrap(),
        "HTTP/1.1 666 Custom status\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\nHello"
    );
}
