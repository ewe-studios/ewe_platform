#![cfg(test)]

//! WebSocket handshake tests (RFC 6455 Section 4).

use base64::Engine;
use foundation_core::wire::simple_http::{SimpleHeader, Status};
use foundation_core::wire::websocket::{
    build_upgrade_request, compute_accept_key, generate_websocket_key, validate_upgrade_response,
};

// Test 1: compute_accept_key with RFC 6455 test vector
#[test]
fn test_compute_accept_key_rfc6455_vector() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    let result = compute_accept_key(client_key);
    assert_eq!(result, expected);
}

// Test 2: generate_websocket_key produces valid base64 encoding of 16 bytes
#[test]
fn test_generate_websocket_key_valid_base64() {
    let key = generate_websocket_key();

    // 16 bytes base64-encoded = 24 characters (with padding)
    assert_eq!(key.len(), 24, "base64-encoded 16 bytes should be 24 chars");

    // Must decode back to exactly 16 bytes
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&key)
        .expect("key should be valid base64");
    assert_eq!(decoded.len(), 16, "decoded key should be 16 bytes");

    // Two generated keys should differ (probabilistic but virtually certain)
    let key2 = generate_websocket_key();
    assert_ne!(key, key2, "two generated keys should differ");
}

/// Helper to get first header value from a `SimpleHeaders` map.
fn first_header_value(
    headers: &foundation_core::wire::simple_http::SimpleHeaders,
    key: &SimpleHeader,
) -> Option<String> {
    headers.get(key).and_then(|v| v.first()).cloned()
}

// Test 3: build_upgrade_request produces correct headers
#[test]
fn test_build_upgrade_request_headers() {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let request =
        build_upgrade_request("example.com", "/chat", key, None).expect("should build request");

    let headers = &request.headers;

    // Verify required WebSocket upgrade headers
    assert_eq!(
        first_header_value(headers, &SimpleHeader::HOST),
        Some("example.com".to_string()),
    );
    assert_eq!(
        first_header_value(headers, &SimpleHeader::UPGRADE),
        Some("websocket".to_string()),
    );
    assert_eq!(
        first_header_value(headers, &SimpleHeader::CONNECTION),
        Some("Upgrade".to_string()),
    );
    assert_eq!(
        first_header_value(headers, &SimpleHeader::SEC_WEBSOCKET_KEY),
        Some(key.to_string()),
    );
    assert_eq!(
        first_header_value(headers, &SimpleHeader::SEC_WEBSOCKET_VERSION),
        Some("13".to_string()),
    );

    // No subprotocol header when None is passed
    assert!(
        headers.get(&SimpleHeader::SEC_WEBSOCKET_PROTOCOL).is_none(),
        "should not have protocol header when None"
    );
}

// Test 4: build_upgrade_request with subprotocols
#[test]
fn test_build_upgrade_request_with_subprotocols() {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let request = build_upgrade_request("example.com", "/chat", key, Some("chat, superchat"))
        .expect("should build request");

    let headers = &request.headers;

    // Verify subprotocol header
    assert_eq!(
        first_header_value(headers, &SimpleHeader::SEC_WEBSOCKET_PROTOCOL),
        Some("chat, superchat".to_string()),
    );
}

// Test 5: validate_upgrade_response with valid response
#[test]
fn test_validate_upgrade_response_valid() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected_accept = compute_accept_key(client_key);

    let mut headers = foundation_core::wire::simple_http::SimpleHeaders::new();
    headers.insert(
        SimpleHeader::SEC_WEBSOCKET_ACCEPT,
        vec![expected_accept.clone()],
    );

    let result = validate_upgrade_response(&Status::SwitchingProtocols, &headers, &expected_accept);
    assert!(result.is_ok());
}

// Test 6: validate_upgrade_response with wrong status code
#[test]
fn test_validate_upgrade_response_wrong_status() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected_accept = compute_accept_key(client_key);

    let headers = foundation_core::wire::simple_http::SimpleHeaders::new();

    let result = validate_upgrade_response(&Status::OK, &headers, &expected_accept);
    assert!(matches!(
        result,
        Err(foundation_core::wire::websocket::WebSocketError::UpgradeFailed(200))
    ));
}

// Test 7: validate_upgrade_response with missing accept key
#[test]
fn test_validate_upgrade_response_missing_accept() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected_accept = compute_accept_key(client_key);

    let headers = foundation_core::wire::simple_http::SimpleHeaders::new();

    let result = validate_upgrade_response(&Status::SwitchingProtocols, &headers, &expected_accept);
    assert!(matches!(
        result,
        Err(foundation_core::wire::websocket::WebSocketError::MissingAcceptKey)
    ));
}

// Test 8: validate_upgrade_response with invalid accept key
#[test]
fn test_validate_upgrade_response_invalid_accept() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected_accept = compute_accept_key(client_key);

    let mut headers = foundation_core::wire::simple_http::SimpleHeaders::new();
    headers.insert(
        SimpleHeader::SEC_WEBSOCKET_ACCEPT,
        vec!["wrong_accept_key".to_string()],
    );

    let result = validate_upgrade_response(&Status::SwitchingProtocols, &headers, &expected_accept);
    assert!(matches!(
        result,
        Err(foundation_core::wire::websocket::WebSocketError::InvalidAcceptKey)
    ));
}
