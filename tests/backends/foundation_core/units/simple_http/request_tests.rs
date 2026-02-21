//! Unit tests for `client::request` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module. They exercise `ClientRequestBuilder`, `PreparedRequest` and the
//! request-building surface in a fast, deterministic manner suitable for unit
//! test execution under `tests/backends/foundation_core/units/simple_http/`.

use foundation_core::wire::simple_http::client::{ClientRequestBuilder, StaticSocketAddr};
use foundation_core::wire::simple_http::{Proto, SendSafeBody, SimpleHeader, SimpleMethod};
use serde::Serialize;
use std::net::SocketAddr as StdSocketAddr;

/// WHY: Verify ClientRequestBuilder::new creates builder
/// WHAT: Tests that new creates a request builder with URL and method
#[test]
fn test_client_request_builder_new() {
    let builder = ClientRequestBuilder::new(SimpleMethod::GET, "http://example.com").unwrap();

    // Consume the builder into a PreparedRequest which exposes public fields for assertions
    let prepared = builder.build().unwrap();

    assert_eq!(prepared.url.host_str().unwrap(), "example.com");
    assert_eq!(prepared.url.port_or_default(), 80);
    assert!(matches!(prepared.method, SimpleMethod::GET));
}

/// WHY: Verify ClientRequestBuilder::new validates URL
/// WHAT: Tests that invalid URLs return error
#[test]
fn test_client_request_builder_new_invalid_url() {
    let result = ClientRequestBuilder::new(SimpleMethod::GET, "not a url");
    assert!(result.is_err());
}

/// WHY: Verify ClientRequestBuilder::header adds header
/// WHAT: Tests that header method adds header to request
#[test]
fn test_client_request_builder_header() {
    let builder = ClientRequestBuilder::get("http://example.com")
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json");

    // Build to obtain a PreparedRequest with public `headers` for assertions
    let prepared = builder.build().unwrap();

    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_TYPE));
    assert_eq!(
        prepared.headers.get(&SimpleHeader::CONTENT_TYPE).unwrap()[0],
        "application/json"
    );
}

/// WHY: Verify ClientRequestBuilder::body_text sets text body
/// WHAT: Tests that body_text sets body and content headers
#[test]
fn test_client_request_builder_body_text() {
    let builder = ClientRequestBuilder::post("http://example.com")
        .unwrap()
        .body_text("Hello, World!");

    // Build to get PreparedRequest with concrete `body` and `headers`
    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Text(_)));
    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_TYPE));
}

/// WHY: Verify ClientRequestBuilder::body_bytes sets binary body
/// WHAT: Tests that body_bytes sets body and content headers
#[test]
fn test_client_request_builder_body_bytes() {
    let builder = ClientRequestBuilder::post("http://example.com")
        .unwrap()
        .body_bytes(vec![1, 2, 3, 4]);

    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Bytes(_)));
    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
}

/// WHY: Verify ClientRequestBuilder::body_json serializes to JSON
/// WHAT: Tests that body_json creates JSON body
#[test]
fn test_client_request_builder_body_json() {
    #[derive(Serialize)]
    struct TestData {
        key: String,
    }

    let data = TestData {
        key: "value".to_string(),
    };

    let builder = ClientRequestBuilder::post("http://example.com")
        .unwrap()
        .body_json(&data)
        .unwrap();

    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Text(_)));
    if let SendSafeBody::Text(json) = &prepared.body {
        assert!(json.contains("\"key\""));
        assert!(json.contains("\"value\""));
    }
}

/// WHY: Verify ClientRequestBuilder::body_form encodes form data
/// WHAT: Tests that body_form creates URL-encoded body
#[test]
fn test_client_request_builder_body_form() {
    let params = vec![
        ("key1".to_string(), "value1".to_string()),
        ("key2".to_string(), "value2".to_string()),
    ];

    let builder = ClientRequestBuilder::post("http://example.com")
        .unwrap()
        .body_form(&params);

    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Text(_)));
    if let SendSafeBody::Text(form) = &prepared.body {
        assert!(form.contains("key1=value1"));
        assert!(form.contains("key2=value2"));
    }
}

/// WHY: Verify ClientRequestBuilder::build creates PreparedRequest
/// WHAT: Tests that build consumes builder and creates prepared request
#[test]
fn test_client_request_builder_build() {
    let prepared = ClientRequestBuilder::get("http://example.com")
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(prepared.url.host_str().unwrap(), "example.com");
    assert!(matches!(prepared.method, SimpleMethod::GET));
    assert!(matches!(prepared.body, SendSafeBody::None));
}

/// WHY: Verify convenience methods create correct builders
/// WHAT: Tests get, post, put, delete, patch, head, options methods
#[test]
fn test_client_request_builder_convenience_methods() {
    let get = ClientRequestBuilder::get("http://example.com").unwrap();
    let prepared_get = get.build().unwrap();
    assert!(matches!(prepared_get.method, SimpleMethod::GET));

    let post = ClientRequestBuilder::post("http://example.com").unwrap();
    let prepared_post = post.build().unwrap();
    assert!(matches!(prepared_post.method, SimpleMethod::POST));

    let put = ClientRequestBuilder::put("http://example.com").unwrap();
    let prepared_put = put.build().unwrap();
    assert!(matches!(prepared_put.method, SimpleMethod::PUT));

    let delete = ClientRequestBuilder::delete("http://example.com").unwrap();
    let prepared_delete = delete.build().unwrap();
    assert!(matches!(prepared_delete.method, SimpleMethod::DELETE));

    let patch = ClientRequestBuilder::patch("http://example.com").unwrap();
    let prepared_patch = patch.build().unwrap();
    assert!(matches!(prepared_patch.method, SimpleMethod::PATCH));

    let head = ClientRequestBuilder::head("http://example.com").unwrap();
    let prepared_head = head.build().unwrap();
    assert!(matches!(prepared_head.method, SimpleMethod::HEAD));

    let options = ClientRequestBuilder::options("http://example.com").unwrap();
    let prepared_options = options.build().unwrap();
    assert!(matches!(prepared_options.method, SimpleMethod::OPTIONS));
}

/// WHY: Verify Host header is automatically added
/// WHAT: Tests that Host header is set from URL
#[test]
fn test_client_request_builder_auto_host_header() {
    let builder = ClientRequestBuilder::get("http://example.com").unwrap();
    let prepared = builder.build().unwrap();
    assert!(prepared.headers.contains_key(&SimpleHeader::HOST));
    assert_eq!(
        prepared.headers.get(&SimpleHeader::HOST).unwrap()[0],
        "example.com"
    );

    let builder2 = ClientRequestBuilder::get("http://example.com:8080").unwrap();
    let prepared2 = builder2.build().unwrap();
    assert_eq!(
        prepared2.headers.get(&SimpleHeader::HOST).unwrap()[0],
        "example.com:8080"
    );
}

/// WHY: Verify PreparedRequest::into_simple_incoming_request works
/// WHAT: Tests that prepared request can be converted to SimpleIncomingRequest
#[test]
fn test_prepared_request_into_simple_incoming_request() {
    let prepared = ClientRequestBuilder::get("http://example.com/path")
        .unwrap()
        .build()
        .unwrap();

    let simple_request = prepared.into_simple_incoming_request().unwrap();
    assert_eq!(simple_request.method, SimpleMethod::GET);
    assert_eq!(simple_request.proto, Proto::HTTP11);
}

/// WHY: Verify PreparedRequest with query string works
/// WHAT: Tests that query strings are preserved
#[test]
fn test_prepared_request_with_query() {
    let prepared = ClientRequestBuilder::get("http://example.com/path?foo=bar")
        .unwrap()
        .build()
        .unwrap();

    let simple_request = prepared.into_simple_incoming_request().unwrap();
    // `request_url` is an internal field on SimpleIncomingRequest that contains the rendered URL.
    // We assert the rendered URL contains the query string.
    assert!(
        simple_request.request_url.url.contains("?foo=bar"),
        "query string must be preserved in rendered request URL"
    );
}

/// WHY: Verify PreparedRequest is Send
/// WHAT: Compile-time test that PreparedRequest can be sent across threads
#[test]
fn test_prepared_request_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<foundation_core::wire::simple_http::client::PreparedRequest>();
}
