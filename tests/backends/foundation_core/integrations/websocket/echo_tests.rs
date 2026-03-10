#![cfg(test)]

//! WebSocket integration tests using real echo server.

use foundation_core::wire::websocket::{WebSocketClient, WebSocketEvent, WebSocketMessage};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_testing::http::WebSocketEchoServer;

/// Test basic text message echo.
#[test]
fn test_text_message_echo() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send text message
    delivery.send(WebSocketMessage::Text("Hello, WebSocket!".to_string())).expect("should send");

    // Receive echoed message
    let response = client.recv().expect("should receive");

    match response {
        WebSocketMessage::Text(text) => {
            assert_eq!(text, "Hello, WebSocket!");
        }
        other => panic!("expected Text message, got: {:?}", other),
    }
}

/// Test binary message echo.
#[test]
fn test_binary_message_echo() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send binary message
    let binary_data = vec![0x01, 0x02, 0x03, 0x04, 0xFF, 0xFE, 0xFD];
    delivery.send(WebSocketMessage::Binary(binary_data.clone())).expect("should send");

    // Receive echoed message
    let response = client.recv().expect("should receive");

    match response {
        WebSocketMessage::Binary(data) => {
            assert_eq!(data, binary_data);
        }
        other => panic!("expected Binary message, got: {:?}", other),
    }
}

/// Test multiple messages in sequence.
#[test]
fn test_multiple_messages_sequence() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    let messages = vec![
        "First message",
        "Second message",
        "Third message with emoji: \u{1F600}",
        "Message with special chars: \u{00E9}\u{00F1}\u{00FC}",
    ];

    for msg in &messages {
        delivery.send(WebSocketMessage::Text(msg.to_string())).expect("should send");

        let response = client.recv().expect("should receive");

        match response {
            WebSocketMessage::Text(text) => {
                assert_eq!(&text, msg);
            }
            other => panic!("expected Text message, got: {:?}", other),
        }
    }
}

/// Test ping/pong exchange.
#[test]
fn test_ping_pong_exchange() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send ping
    let ping_data = vec![0x01, 0x02, 0x03];
    delivery.send(WebSocketMessage::Ping(ping_data.clone())).expect("should send ping");

    // Receive pong response
    let response = client.recv().expect("should receive");

    match response {
        WebSocketMessage::Pong(data) => {
            assert_eq!(data, ping_data);
        }
        other => panic!("expected Pong message, got: {:?}", other),
    }
}

/// Test client-initiated close handshake.
#[test]
fn test_client_initiated_close() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send close frame
    delivery.close(1000, "goodbye").expect("should send close");

    // Server echoes close frame back, we should receive it
    let response = client.recv().expect("should receive close response");

    match response {
        WebSocketMessage::Close(code, reason) => {
            assert_eq!(code, 1000);
            assert_eq!(reason, "goodbye");
        }
        other => panic!("expected Close message, got: {:?}", other),
    }
}

/// Test large message handling.
#[test]
fn test_large_message_echo() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send large message (triggers 2-byte extended length)
    let large_text = "A".repeat(500);
    delivery.send(WebSocketMessage::Text(large_text.clone())).expect("should send");

    // Receive echoed message
    let response = client.recv().expect("should receive");

    match response {
        WebSocketMessage::Text(text) => {
            assert_eq!(text, large_text);
            assert_eq!(text.len(), 500);
        }
        other => panic!("expected Text message, got: {:?}", other),
    }
}

/// Test very large message (4-byte extended length).
#[test]
fn test_very_large_message_echo() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send very large message (triggers 4-byte extended length, > 65535 bytes)
    let large_text = "B".repeat(70_000);
    delivery.send(WebSocketMessage::Text(large_text.clone())).expect("should send");

    // Receive echoed message
    let response = client.recv().expect("should receive");

    match response {
        WebSocketMessage::Text(text) => {
            assert_eq!(text, large_text);
            assert_eq!(text.len(), 70_000);
        }
        other => panic!("expected Text message, got: {:?}", other),
    }
}

/// Test UTF-8 validation.
#[test]
fn test_utf8_text_messages() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    let utf8_messages = vec![
        "Hello, world!",
        "Unicode: \u{4F00}\u{540D}\u{66F8}",  // Chinese characters
        "Emoji: \u{1F600}\u{1F601}\u{1F602}",  // Emojis
        "Accents: \u{00E9}\u{00E8}\u{00EA}",   // French accents
        "Mixed: Hello \u{4E16}\u{754C} \u{1F30D}!", // Mixed script
    ];

    for msg in &utf8_messages {
        delivery.send(WebSocketMessage::Text(msg.to_string())).expect("should send");

        let response = client.recv().expect("should receive");

        match response {
            WebSocketMessage::Text(text) => {
                assert_eq!(&text, msg, "UTF-8 message should echo correctly");
            }
            other => panic!("expected Text message, got: {:?}", other),
        }
    }
}

/// Test message iterator.
#[test]
fn test_message_iterator() {
    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(SystemDnsResolver::default(), url).expect("should connect");

    // Send a few messages
    let messages = vec!["msg1", "msg2", "msg3"];
    for msg in &messages {
        delivery.send(WebSocketMessage::Text(msg.to_string())).expect("should send");
    }

    // Use iterator to receive messages
    let mut received = Vec::new();
    for result in client.messages().take(9) { // Take more to account for Skip events
        match result.expect("should receive") {
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => received.push(text),
            WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
            WebSocketEvent::Skip => continue,
        }
        if received.len() >= messages.len() {
            break;
        }
    }

    assert_eq!(received, messages);
}
