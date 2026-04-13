//! Unit tests for `client::request` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module. They exercise `ClientRequestBuilder::<SystemDnsResolver>`, `PreparedRequest` and the
//! request-building surface in a fast, deterministic manner suitable for unit
//! test execution under `tests/backends/foundation_core/units/simple_http/`.

use foundation_core::wire::simple_http::client::{
    ClientRequestBuilder, StaticSocketAddr, SystemDnsResolver,
};
use foundation_core::wire::simple_http::{Proto, SendSafeBody, SimpleHeader, SimpleMethod};
use serde::Serialize;
use std::net::SocketAddr as StdSocketAddr;

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::new` creates builder
/// WHAT: Tests that new creates a request builder with URL and method
#[test]
fn test_client_request_builder_new() {
    let builder =
        ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::GET, "http://example.com")
            .unwrap();

    // Consume the builder into a PreparedRequest which exposes public fields for assertions
    let prepared = builder.build().unwrap();

    assert_eq!(prepared.url.host_str().unwrap(), "example.com");
    assert_eq!(prepared.url.port_or_default(), 80);
    assert!(matches!(prepared.method, SimpleMethod::GET));
}

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::new` validates URL
/// WHAT: Tests that invalid URLs return error
#[test]
fn test_client_request_builder_new_invalid_url() {
    let result = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::GET, "not a url");
    assert!(result.is_err());
}

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::header` adds header
/// WHAT: Tests that header method adds header to request
#[test]
fn test_client_request_builder_header() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
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

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::body_text` sets text body
/// WHAT: Tests that `body_text` sets body and content headers
#[test]
fn test_client_request_builder_body_text() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::post("http://example.com")
        .unwrap()
        .body_text("Hello, World!");

    // Build to get PreparedRequest with concrete `body` and `headers`
    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Text(_)));
    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_TYPE));
}

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::body_bytes` sets binary body
/// WHAT: Tests that `body_bytes` sets body and content headers
#[test]
fn test_client_request_builder_body_bytes() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::post("http://example.com")
        .unwrap()
        .body_bytes(vec![1, 2, 3, 4]);

    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Bytes(_)));
    assert!(prepared.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
}

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::body_json` serializes to JSON
/// WHAT: Tests that `body_json` creates JSON body
#[test]
fn test_client_request_builder_body_json() {
    #[derive(Serialize)]
    struct TestData {
        key: String,
    }

    let data = TestData {
        key: "value".to_string(),
    };

    let builder = ClientRequestBuilder::<SystemDnsResolver>::post("http://example.com")
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

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::body_form` encodes form data
/// WHAT: Tests that `body_form` creates URL-encoded body
#[test]
fn test_client_request_builder_body_form() {
    let params = vec![
        ("key1".to_string(), "value1".to_string()),
        ("key2".to_string(), "value2".to_string()),
    ];

    let builder = ClientRequestBuilder::<SystemDnsResolver>::post("http://example.com")
        .unwrap()
        .body_form(&params);

    let prepared = builder.build().unwrap();

    assert!(matches!(prepared.body, SendSafeBody::Text(_)));
    if let SendSafeBody::Text(form) = &prepared.body {
        assert!(form.contains("key1=value1"));
        assert!(form.contains("key2=value2"));
    }
}

/// WHY: Verify `ClientRequestBuilder::`<SystemDnsResolver>`::build` creates `PreparedRequest`
/// WHAT: Tests that build consumes builder and creates prepared request
#[test]
fn test_client_request_builder_build() {
    let prepared = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
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
    let get = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com").unwrap();
    let prepared_get = get.build().unwrap();
    assert!(matches!(prepared_get.method, SimpleMethod::GET));

    let post = ClientRequestBuilder::<SystemDnsResolver>::post("http://example.com").unwrap();
    let prepared_post = post.build().unwrap();
    assert!(matches!(prepared_post.method, SimpleMethod::POST));

    let put = ClientRequestBuilder::<SystemDnsResolver>::put("http://example.com").unwrap();
    let prepared_put = put.build().unwrap();
    assert!(matches!(prepared_put.method, SimpleMethod::PUT));

    let delete = ClientRequestBuilder::<SystemDnsResolver>::delete("http://example.com").unwrap();
    let prepared_delete = delete.build().unwrap();
    assert!(matches!(prepared_delete.method, SimpleMethod::DELETE));

    let patch = ClientRequestBuilder::<SystemDnsResolver>::patch("http://example.com").unwrap();
    let prepared_patch = patch.build().unwrap();
    assert!(matches!(prepared_patch.method, SimpleMethod::PATCH));

    let head = ClientRequestBuilder::<SystemDnsResolver>::head("http://example.com").unwrap();
    let prepared_head = head.build().unwrap();
    assert!(matches!(prepared_head.method, SimpleMethod::HEAD));

    let options = ClientRequestBuilder::<SystemDnsResolver>::options("http://example.com").unwrap();
    let prepared_options = options.build().unwrap();
    assert!(matches!(prepared_options.method, SimpleMethod::OPTIONS));
}

/// WHY: Verify Host header is automatically added
/// WHAT: Tests that Host header is set from URL
#[test]
fn test_client_request_builder_auto_host_header() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com").unwrap();
    let prepared = builder.build().unwrap();
    assert!(prepared.headers.contains_key(&SimpleHeader::HOST));
    assert_eq!(
        prepared.headers.get(&SimpleHeader::HOST).unwrap()[0],
        "example.com"
    );

    let builder2 =
        ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com:8080").unwrap();
    let prepared2 = builder2.build().unwrap();
    assert_eq!(
        prepared2.headers.get(&SimpleHeader::HOST).unwrap()[0],
        "example.com:8080"
    );
}

/// WHY: Verify `PreparedRequest::into_simple_incoming_request` works
/// WHAT: Tests that prepared request can be converted to `SimpleIncomingRequest`
#[test]
fn test_prepared_request_into_simple_incoming_request() {
    let prepared = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com/path")
        .unwrap()
        .build()
        .unwrap();

    let simple_request = prepared.into_simple_incoming_request().unwrap();
    assert_eq!(simple_request.method, SimpleMethod::GET);
    assert_eq!(simple_request.proto, Proto::HTTP11);
}

/// WHY: Verify `PreparedRequest` with query string works
/// WHAT: Tests that query strings are preserved
#[test]
fn test_prepared_request_with_query() {
    let prepared =
        ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com/path?foo=bar")
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

/// WHY: Verify `PreparedRequest` is Send
/// WHAT: Compile-time test that `PreparedRequest` can be sent across threads
#[test]
fn test_prepared_request_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<foundation_core::wire::simple_http::client::PreparedRequest>();
}

/// WHY: Basic auth must encode credentials correctly per RFC 7617
/// WHAT: Test that `basic_auth()` creates proper Authorization header with base64 encoding
#[test]
fn test_basic_auth_encodes_credentials() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
        .unwrap()
        .basic_auth("username", "password");

    let prepared = builder.build().unwrap();
    let auth_header = prepared
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .expect("Authorization header should be present");

    // "username:password" encodes to "dXNlcm5hbWU6cGFzc3dvcmQ="
    assert_eq!(auth_header, &vec!["Basic dXNlcm5hbWU6cGFzc3dvcmQ="]);
}

/// WHY: Bearer token auth must format token correctly per RFC 6750
/// WHAT: Test that `bearer_token()` creates proper Authorization header with Bearer prefix
#[test]
fn test_bearer_token_formats_correctly() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
        .unwrap()
        .bearer_token("my-jwt-token-123");

    let prepared = builder.build().unwrap();
    let auth_header = prepared
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .expect("Authorization header should be present");

    assert_eq!(auth_header, &vec!["Bearer my-jwt-token-123"]);
}

/// WHY: `bearer_auth` is an alias for `bearer_token` for convenience
/// WHAT: Test that `bearer_auth()` works identically to `bearer_token()`
#[test]
fn test_bearer_auth_alias() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
        .unwrap()
        .bearer_auth("alias-token");

    let prepared = builder.build().unwrap();
    let auth_header = prepared
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .expect("Authorization header should be present");

    assert_eq!(auth_header, &vec!["Bearer alias-token"]);
}

/// WHY: API key authentication is common pattern for REST APIs
/// WHAT: Test that `api_key()` sets custom header with key value
#[test]
fn test_api_key_custom_header() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
        .unwrap()
        .api_key("X-API-Key", "secret-key-123");

    let prepared = builder.build().unwrap();

    // Check the custom header exists
    let headers_map = &prepared.headers;
    let custom_header = SimpleHeader::from("X-API-Key".to_string());
    let key_header = headers_map
        .get(&custom_header)
        .expect("X-API-Key header should be present");

    assert_eq!(key_header, &vec!["secret-key-123"]);
}

/// WHY: X-API-Key is very common header name for API authentication
/// WHAT: Test that `x_api_key()` convenience method sets X-API-Key header
#[test]
fn test_x_api_key_convenience() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
        .unwrap()
        .x_api_key("my-key");

    let prepared = builder.build().unwrap();

    let custom_header = SimpleHeader::from("X-API-Key".to_string());
    let key_header = prepared
        .headers
        .get(&custom_header)
        .expect("X-API-Key header should be present");

    assert_eq!(key_header, &vec!["my-key"]);
}

/// WHY: Custom auth schemes like Digest, AWS4-HMAC-SHA256 need flexible format
/// WHAT: Test that `authorization()` formats custom scheme and credentials correctly
#[test]
fn test_authorization_custom_scheme() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
        .unwrap()
        .authorization("CustomScheme", "token123");

    let prepared = builder.build().unwrap();
    let auth_header = prepared
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .expect("Authorization header should be present");

    assert_eq!(auth_header, &vec!["CustomScheme token123"]);
}

/// WHY: Basic auth should handle optional password (empty string if None per RFC 7617)
/// WHAT: Test that `basic_auth_opt()` works with Some password
#[test]
fn test_basic_auth_opt_with_password() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
        .unwrap()
        .basic_auth_opt("user", Some("pass"));

    let prepared = builder.build().unwrap();
    let auth_header = prepared
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .expect("Authorization header should be present");

    // "user:pass" encodes to "dXNlcjpwYXNz"
    assert_eq!(auth_header, &vec!["Basic dXNlcjpwYXNz"]);
}

/// WHY: Basic auth with None password should use empty string per RFC 7617
/// WHAT: Test that `basic_auth_opt()` with None password encodes as "username:"
#[test]
fn test_basic_auth_opt_without_password() {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
        .unwrap()
        .basic_auth_opt("user", None);

    let prepared = builder.build().unwrap();
    let auth_header = prepared
        .headers
        .get(&SimpleHeader::AUTHORIZATION)
        .expect("Authorization header should be present");

    // "user:" encodes to "dXNlcjo="
    assert_eq!(auth_header, &vec!["Basic dXNlcjo="]);
}
