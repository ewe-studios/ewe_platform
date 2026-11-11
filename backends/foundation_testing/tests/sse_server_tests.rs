//! `SseTestServer` tests moved out of `src/http/sse_server.rs` (spec 70
//! feature 59, rule 7). Every item the tests use is public.

use std::io::{BufReader, Read, Write};
use std::net::TcpStream;

use foundation_testing::http::{HttpRequest, SseConnectionResult, SseStreamWriter, SseTestServer};

#[test]
fn test_server_start() {
    let server = SseTestServer::events_then_close(1);
    assert!(server.base_url().starts_with("http://127.0.0.1:"));
    assert_eq!(server.connect_count(), 0);
}

#[test]
fn test_server_drop() {
    {
        let _server = SseTestServer::events_then_close(1);
    }
    // Server should have stopped cleanly
}

#[test]
fn test_custom_handler() {
    let server = SseTestServer::with_handler(
        |_req: &HttpRequest, _writer: &mut SseStreamWriter, _conn: usize| {
            SseConnectionResult::clean_close()
        },
    );
    assert!(server.base_url().starts_with("http://"));
}

/// End-to-end: connect via raw TCP and verify SSE events are received.
#[test]
fn test_sse_events_received() {
    let server = SseTestServer::events_then_close(2);
    let port = server
        .base_url()
        .strip_prefix("http://127.0.0.1:")
        .unwrap()
        .parse::<u16>()
        .unwrap();

    // Connect with raw TCP
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;

    // Send a minimal HTTP/1.1 GET request
    write!(writer, "GET /test HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    writer.flush().unwrap();

    // Read the full response
    let mut response = String::new();
    reader.read_to_string(&mut response).unwrap();

    // Should contain HTTP 200 + SSE events
    assert!(response.contains("200 OK"));
    assert!(response.contains("data: {\"seq\": 0}"));
    assert!(response.contains("data: {\"seq\": 1}"));
}
