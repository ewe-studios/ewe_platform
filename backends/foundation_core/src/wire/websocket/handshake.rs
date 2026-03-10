//! WebSocket handshake implementation (RFC 6455 Section 4).
//!
//! WHY: WebSocket connections begin with an HTTP upgrade handshake that must follow
//! strict protocol rules for key generation and validation.
//! WHAT: Provides functions to generate WebSocket keys, build upgrade requests,
//! and validate upgrade responses per RFC 6455.
//! HOW: Uses SHA-1 hashing and Base64 encoding for the Sec-WebSocket-Accept
//! key derivation, and `SimpleIncomingRequestBuilder` for HTTP request construction.

use base64::Engine;
use sha1::{Digest, Sha1};

use super::error::WebSocketError;
use crate::wire::simple_http::{
    SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleIncomingRequestBuilder, SimpleMethod,
    Status,
};

/// The magic GUID defined in RFC 6455 Section 4.2.2 for Sec-WebSocket-Accept computation.
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// WHY: The server must prove it received the client's key by computing a specific accept value.
///
/// WHAT: Computes the Sec-WebSocket-Accept value from a client's Sec-WebSocket-Key.
///
/// HOW: Concatenates the client key with the RFC 6455 GUID, SHA-1 hashes the result,
/// and Base64 encodes the hash.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn compute_accept_key(client_key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WEBSOCKET_GUID.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// WHY: Each WebSocket handshake requires a unique random key for security.
///
/// WHAT: Generates a random 16-byte Sec-WebSocket-Key, Base64 encoded.
///
/// HOW: Uses the `rand` crate to generate 16 random bytes, then Base64 encodes them.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn generate_websocket_key() -> String {
    let mut bytes = [0u8; 16];
    rand::fill(&mut bytes);
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// WHY: The client must send a properly formatted HTTP upgrade request to initiate WebSocket.
///
/// WHAT: Builds an HTTP GET request with all required WebSocket upgrade headers.
///
/// HOW: Uses `SimpleIncomingRequestBuilder` to construct a request with Host, Upgrade,
/// Connection, Sec-WebSocket-Key, and Sec-WebSocket-Version headers.
///
/// # Arguments
/// * `host` - The host header value (e.g., "example.com" or "example.com:8080")
/// * `path` - The request path (e.g., "/chat")
/// * `key` - The Sec-WebSocket-Key value (from `generate_websocket_key`)
/// * `protocols` - Optional subprotocol list (e.g., `Some("chat, superchat")`)
///
/// # Errors
/// Returns [`WebSocketError::InvalidUrl`] if the request cannot be built.
///
/// # Panics
/// Never panics.
pub fn build_upgrade_request(
    host: &str,
    path: &str,
    key: &str,
    protocols: Option<&str>,
) -> Result<SimpleIncomingRequest, WebSocketError> {
    let url = format!("http://{host}{path}");

    let mut builder = SimpleIncomingRequestBuilder::default()
        .with_plain_url(url)
        .with_method(SimpleMethod::GET)
        .add_header(SimpleHeader::HOST, host)
        .add_header(SimpleHeader::UPGRADE, "websocket")
        .add_header(SimpleHeader::CONNECTION, "Upgrade")
        .add_header(SimpleHeader::SEC_WEBSOCKET_KEY, key)
        .add_header(SimpleHeader::SEC_WEBSOCKET_VERSION, "13");

    if let Some(protos) = protocols {
        builder = builder.add_header(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, protos);
    }

    builder
        .build()
        .map_err(|e| WebSocketError::InvalidUrl(format!("failed to build upgrade request: {e}")))
}

/// WHY: The client must validate the server's 101 response to confirm a successful upgrade.
///
/// WHAT: Validates that an HTTP response is a valid WebSocket upgrade response.
///
/// HOW: Checks that the status is 101 Switching Protocols and that the
/// Sec-WebSocket-Accept header contains the correct derived value.
///
/// # Arguments
/// * `status` - The HTTP response status code
/// * `headers` - The HTTP response headers
/// * `expected_accept` - The expected Sec-WebSocket-Accept value (computed from the client key)
///
/// # Errors
/// Returns [`WebSocketError::UpgradeFailed`] if status is not 101.
/// Returns [`WebSocketError::MissingAcceptKey`] if the accept header is missing.
/// Returns [`WebSocketError::InvalidAcceptKey`] if the accept header value is wrong.
///
/// # Panics
/// Never panics.
pub fn validate_upgrade_response(
    status: &Status,
    headers: &SimpleHeaders,
    expected_accept: &str,
) -> Result<(), WebSocketError> {
    // Check status is 101 Switching Protocols
    if *status != Status::SwitchingProtocols {
        #[allow(clippy::cast_possible_truncation)] // HTTP status codes fit in u16
        return Err(WebSocketError::UpgradeFailed(
            status.clone().into_usize() as u16,
        ));
    }

    // Check Sec-WebSocket-Accept header
    let accept_values = headers
        .get(&SimpleHeader::SEC_WEBSOCKET_ACCEPT)
        .ok_or(WebSocketError::MissingAcceptKey)?;

    let accept_value = accept_values
        .first()
        .ok_or(WebSocketError::MissingAcceptKey)?;

    if accept_value != expected_accept {
        return Err(WebSocketError::InvalidAcceptKey);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn first_header_value(headers: &SimpleHeaders, key: &SimpleHeader) -> Option<String> {
        headers.get(key).and_then(|v| v.first()).cloned()
    }

    // Test 3: build_upgrade_request produces correct headers
    #[test]
    fn test_build_upgrade_request_headers() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let request = build_upgrade_request("example.com", "/chat", key, None)
            .expect("should build request");

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
}
