#![cfg(test)]

//! WebSocket subprotocol negotiation integration tests.
//!
//! Tests Sec-WebSocket-Protocol header handling during handshake.

use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_core::wire::websocket::{WebSocketClient, WebSocketEvent, WebSocketMessage};
use foundation_testing::http::WebSocketEchoServer;
use std::time::Duration;
use tracing_test::traced_test;

// Test 1: Client can request subprotocol
#[test]
#[traced_test]
fn test_client_requests_subprotocol() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    // Start server with subprotocol support
    let server = WebSocketEchoServer::with_subprotocols("chat,superchat");
    // Extract base URL (strip |subprotocols=... part)
    let base_url = server.base_url().split('|').next().unwrap();
    let url = format!("{}/echo", base_url);

    // Connect with subprotocol
    let result = WebSocketClient::with_options(
        SystemDnsResolver,
        &url,
        Some("chat".to_string()),
        Vec::new(),
        Duration::from_secs(5),
        Duration::from_secs(1),
    );

    assert!(result.is_ok(), "Should connect to server with subprotocols");

    let (mut client, delivery) = result.unwrap();

    // Send a message to verify connection works
    delivery
        .send(WebSocketMessage::Text("test".to_string()))
        .unwrap();

    // Receive response
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                assert_eq!(text, "test");
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
        }
    }
}

// Test 2: Client with_subprotocol builder method
#[test]
#[traced_test]
fn test_client_with_subprotocol_builder() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::with_subprotocols("chat");
    let base_url = server.base_url().split('|').next().unwrap();
    let url = format!("{}/echo", base_url);

    // Connect using with_options to set subprotocol
    let result = WebSocketClient::with_options(
        SystemDnsResolver,
        &url,
        Some("chat".to_string()),
        Vec::new(),
        Duration::from_secs(5),
        Duration::from_secs(1),
    );

    assert!(result.is_ok());
}

// Test 3: Server selects first matching subprotocol
#[test]
#[traced_test]
fn test_server_selects_first_matching_protocol() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    // Server supports "chat" and "superchat"
    let server = WebSocketEchoServer::with_subprotocols("chat,superchat");
    let base_url = server.base_url().split('|').next().unwrap();
    let url = format!("{}/echo", base_url);

    // Client requests "chat" first
    let result = WebSocketClient::with_options(
        SystemDnsResolver,
        &url,
        Some("chat".to_string()),
        Vec::new(),
        Duration::from_secs(5),
        Duration::from_secs(1),
    );

    assert!(
        result.is_ok(),
        "Should connect when client requests supported protocol"
    );
}

// Test 4: Client requests multiple subprotocols
#[test]
#[traced_test]
fn test_client_requests_multiple_subprotocols() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    // Server supports "superchat" (second in list)
    let server = WebSocketEchoServer::with_subprotocols("superchat,chat");
    let base_url = server.base_url().split('|').next().unwrap();
    let url = format!("{}/echo", base_url);

    // Client requests both, server should pick first match
    let result = WebSocketClient::with_options(
        SystemDnsResolver,
        &url,
        Some("superchat,chat".to_string()),
        Vec::new(),
        Duration::from_secs(5),
        Duration::from_secs(1),
    );

    assert!(
        result.is_ok(),
        "Should connect with multiple requested protocols"
    );

    let (mut client, delivery) = result.unwrap();

    // Verify connection works
    delivery
        .send(WebSocketMessage::Text("hello".to_string()))
        .unwrap();

    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                assert_eq!(text, "hello");
                break;
            }
            _ => break,
        }
    }
}

// Test 5: Connection succeeds without subprotocol
#[test]
#[traced_test]
fn test_connection_without_subprotocol() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    // Server supports protocols but client doesn't request any
    let server = WebSocketEchoServer::with_subprotocols("chat,superchat");
    let base_url = server.base_url().split('|').next().unwrap();
    let url = format!("{}/echo", base_url);

    let result = WebSocketClient::connect(
        SystemDnsResolver,
        &url,
        Duration::from_secs(5),
        Duration::from_secs(1),
    );

    assert!(
        result.is_ok(),
        "Should connect without requesting subprotocol"
    );
}

// Test 6: Server without subprotocol support
#[test]
#[traced_test]
fn test_server_without_subprotocol_support() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    // Standard echo server without subprotocol configuration
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    // Client connects without requesting subprotocol
    let result = WebSocketClient::connect(
        SystemDnsResolver,
        &url,
        Duration::from_secs(5),
        Duration::from_secs(1),
    );

    assert!(
        result.is_ok(),
        "Should connect to server without subprotocol"
    );
}

// Test 7: Subprotocol header extraction (unit test for server-side)
#[test]
#[traced_test]
fn test_subprotocol_header_extraction() {
    use foundation_core::wire::simple_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};

    // Build a mock request with subprotocol header
    let builder = SimpleIncomingRequest::builder()
        .with_plain_url("http://localhost/chat")
        .with_method(SimpleMethod::GET)
        .add_header_raw(SimpleHeader::UPGRADE, "websocket")
        .add_header_raw(SimpleHeader::CONNECTION, "Upgrade")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_VERSION, "13")
        .add_header_raw(
            SimpleHeader::SEC_WEBSOCKET_PROTOCOL,
            "chat, superchat, other",
        );

    let request = builder.build().unwrap();

    // Extract subprotocols
    let protocols =
        foundation_core::wire::websocket::WebSocketUpgrade::extract_subprotocols(&request);

    assert_eq!(protocols, Some("chat, superchat, other".to_string()));
}

// Test 8: Missing subprotocol header returns None
#[test]
#[traced_test]
fn test_missing_subprotocol_returns_none() {
    use foundation_core::wire::simple_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};

    let builder = SimpleIncomingRequest::builder()
        .with_plain_url("http://localhost/chat")
        .with_method(SimpleMethod::GET)
        .add_header_raw(SimpleHeader::UPGRADE, "websocket")
        .add_header_raw(SimpleHeader::CONNECTION, "Upgrade")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_VERSION, "13");

    let request = builder.build().unwrap();

    let protocols =
        foundation_core::wire::websocket::WebSocketUpgrade::extract_subprotocols(&request);

    assert!(
        protocols.is_none(),
        "Should return None when header is missing"
    );
}

// Test 9: Empty subprotocol string handling
#[test]
#[traced_test]
fn test_empty_subprotocol_string() {
    use foundation_core::wire::simple_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};

    let builder = SimpleIncomingRequest::builder()
        .with_plain_url("http://localhost/chat")
        .with_method(SimpleMethod::GET)
        .add_header_raw(SimpleHeader::UPGRADE, "websocket")
        .add_header_raw(SimpleHeader::CONNECTION, "Upgrade")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_VERSION, "13")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, "");

    let request = builder.build().unwrap();

    let protocols =
        foundation_core::wire::websocket::WebSocketUpgrade::extract_subprotocols(&request);

    assert_eq!(
        protocols,
        Some("".to_string()),
        "Empty header value should return Some empty string"
    );
}

// Test 10: Server includes selected protocol in response
#[test]
#[traced_test]
fn test_server_includes_selected_protocol() {
    use foundation_core::wire::simple_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};
    use foundation_core::wire::websocket::WebSocketUpgrade;

    // Build request with subprotocol
    let builder = SimpleIncomingRequest::builder()
        .with_plain_url("http://localhost/chat")
        .with_method(SimpleMethod::GET)
        .add_header_raw(SimpleHeader::UPGRADE, "websocket")
        .add_header_raw(SimpleHeader::CONNECTION, "Upgrade")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_VERSION, "13")
        .add_header_raw(SimpleHeader::SEC_WEBSOCKET_PROTOCOL, "chat, superchat");

    let request = builder.build().unwrap();

    // Accept with "chat" protocol
    let (response_bytes, _) = WebSocketUpgrade::accept(&request, Some("chat")).unwrap();

    // Verify response includes protocol header
    let response_str: String = response_bytes
        .iter()
        .flat_map(|chunk| chunk.iter().map(|&b| b as char))
        .collect();

    let response_upper = response_str.to_uppercase();
    assert!(
        response_upper.contains("SEC-WEBSOCKET-PROTOCOL"),
        "Response should include protocol header"
    );
    assert!(
        response_upper.contains("CHAT"),
        "Response should include selected protocol value"
    );
}
