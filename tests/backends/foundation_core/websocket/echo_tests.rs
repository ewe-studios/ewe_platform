#![cfg(test)]

//! WebSocket integration tests using real echo server.

use std::time::Duration;

use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_core::wire::websocket::{WebSocketClient, WebSocketEvent, WebSocketMessage};
use foundation_testing::http::WebSocketEchoServer;
use tracing_test::traced_test;

/// Test basic text message echo.
#[test]
#[traced_test]
fn test_text_message_echo() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(4),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send text message via MessageDelivery
    delivery
        .send(WebSocketMessage::Text("Hello, WebSocket!".to_string()))
        .expect("should send");

    // Receive echoed message via iterator
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                assert_eq!(text, "Hello, WebSocket!");
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
        }
    }
}

/// Test binary message echo.
#[test]
#[traced_test]
fn test_binary_message_echo() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send binary message
    let binary_data = vec![0x01, 0x02, 0x03, 0x04, 0xFF, 0xFE, 0xFD];
    delivery
        .send(WebSocketMessage::Binary(binary_data.clone()))
        .expect("should send");

    // Receive echoed message
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Binary(data)) => {
                assert_eq!(data, binary_data);
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Binary message, got: {:?}", other),
        }
    }
}

/// Test multiple messages in sequence.
#[test]
#[traced_test]
fn test_multiple_messages_sequence() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_millis(10),
    )
    .expect("should connect");

    let messages = vec![
        "First message",
        "Second message",
        "Third message with emoji: \u{1F600}",
        "Message with special chars: \u{00E9}\u{00F1}\u{00FC}",
    ];

    for msg in &messages {
        delivery
            .send(WebSocketMessage::Text(msg.to_string()))
            .expect("should send");

        tracing::info!("Sent message: {}", msg);
        for event in client.messages() {
            tracing::info!("Received message: {:?}", &event);
            match event.expect("should receive event") {
                WebSocketEvent::Skip => {
                    tracing::info!("Got skip signal");
                    continue;
                }
                WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                    assert_eq!(&text, msg);
                    break;
                }
                WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
            }
        }
    }
    println!("Finished, no more work")
}

/// Test ping/pong exchange.
#[test]
#[traced_test]
fn test_ping_pong_exchange() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send ping
    let ping_data = vec![0x01, 0x02, 0x03];
    delivery
        .send(WebSocketMessage::Ping(ping_data.clone()))
        .expect("should send ping");

    // Receive pong response
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Pong(data)) => {
                assert_eq!(data, ping_data);
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Pong message, got: {:?}", other),
        }
    }
}

/// Test client-initiated close handshake.
#[test]
#[traced_test]
fn test_client_initiated_close() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send close frame
    delivery.close(1000, "goodbye").expect("should send close");

    // Server echoes close frame back, we should receive it
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Close(code, reason)) => {
                assert_eq!(code, 1000);
                assert_eq!(reason, "goodbye");
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Close message, got: {:?}", other),
        }
    }
}

/// Test large message handling.
#[test]
#[traced_test]
fn test_large_message_echo() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send large message (triggers 2-byte extended length)
    let large_text = "A".repeat(500);
    delivery
        .send(WebSocketMessage::Text(large_text.clone()))
        .expect("should send");

    // Receive echoed message
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                assert_eq!(text, large_text);
                assert_eq!(text.len(), 500);
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
        }
    }
}

/// Test very large message (4-byte extended length).
#[test]
#[traced_test]
fn test_very_large_message_echo() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send very large message (triggers 4-byte extended length, > 65535 bytes)
    let large_text = "B".repeat(70_000);
    delivery
        .send(WebSocketMessage::Text(large_text.clone()))
        .expect("should send");

    // Receive echoed message
    for event in client.messages() {
        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                assert_eq!(text, large_text);
                assert_eq!(text.len(), 70_000);
                break;
            }
            WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
        }
    }
}

/// Test UTF-8 validation.
#[test]
#[traced_test]
fn test_utf8_text_messages() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(3),
        Duration::from_secs(1),
    )
    .expect("should connect");

    let utf8_messages = vec![
        "Hello, world!",
        "Unicode: \u{4F00}\u{540D}\u{66F8}", // Chinese characters
        "Emoji: \u{1F600}\u{1F601}\u{1F602}", // Emojis
        "Accents: \u{00E9}\u{00E8}\u{00EA}", // French accents
        "Mixed: Hello \u{4E16}\u{754C} \u{1F30D}!", // Mixed script
    ];

    for msg in &utf8_messages {
        delivery
            .send(WebSocketMessage::Text(msg.to_string()))
            .expect("should send");

        for event in client.messages() {
            match event.expect("should receive event") {
                WebSocketEvent::Skip => continue,
                WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                    assert_eq!(&text, msg, "UTF-8 message should echo correctly");
                    break;
                }
                WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
            }
        }
    }
}

/// Test message iterator.
#[test]
#[traced_test]
fn test_message_iterator() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let server = WebSocketEchoServer::start();
    let url = server.ws_url("/echo");

    let (mut client, delivery) = WebSocketClient::connect(
        SystemDnsResolver,
        url,
        Duration::from_secs(2),
        Duration::from_secs(1),
    )
    .expect("should connect");

    // Send a few messages
    let messages = vec!["msg1", "msg2", "msg3"];
    for msg in &messages {
        delivery
            .send(WebSocketMessage::Text(msg.to_string()))
            .expect("should send");
    }

    // Use iterator to receive messages with time-based timeout
    let mut received = Vec::new();
    let timeout = std::time::Duration::from_secs(180); // 3 minute timeout
    let start = std::time::Instant::now();

    for event in client.messages() {
        // Check for timeout first
        if start.elapsed() > timeout {
            tracing::warn!("Timeout elapsed ({:?}), breaking", timeout);
            break;
        }

        match event.expect("should receive event") {
            WebSocketEvent::Skip => continue,
            WebSocketEvent::Message(WebSocketMessage::Text(text)) => {
                tracing::info!("Received text: {}", text);
                received.push(text);
                if received.len() >= messages.len() {
                    tracing::info!("Received all {} messages, breaking", received.len());
                    break;
                }
            }
            WebSocketEvent::Message(other) => panic!("expected Text message, got: {:?}", other),
        }
    }

    tracing::info!(
        "Final: received {:?}, expected {:?}, elapsed: {:?}",
        received,
        messages,
        start.elapsed()
    );
    assert_eq!(received, messages);
}
