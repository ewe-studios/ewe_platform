//! Integration test for HTTP client: redirect limit enforcement
//! This test spins up a local HTTP server that responds with 302 (redirect)
//! and checks that the client triggers TooManyRedirects when max_redirects is exceeded.

use foundation_core::wire::simple_http::client::*;
use foundation_core::wire::simple_http::*;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

fn spawn_redirect_server() -> u16 {
    // Bind to an available port
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
    let port = listener.local_addr().unwrap().port();
    // Start a thread for the server
    thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buffer = [0u8; 512];
                let _ = stream.read(&mut buffer);
                // Always reply with 302 redirect to /redirect (itself)
                let response = "HTTP/1.1 302 Found\r\n\
                                Location: /redirect\r\n\
                                Content-Length: 0\r\n\
                                Connection: close\r\n\r\n";
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
    port
}

#[test]
fn redirect_limit_triggers_too_many_redirects() {
    // Spin up local redirect server
    let port = spawn_redirect_server();

    // Allow server a moment to start
    std::thread::sleep(Duration::from_millis(100));

    // Configure the HTTP client (system DNS resolver and zero redirects allowed)
    let client = SimpleHttpClient::from_system().max_redirects(0);

    let url = format!("http://127.0.0.1:{}/redirect", port);

    // Create a request
    let request_result = client.get(&url);
    assert!(request_result.is_ok(), "Failed to build request");
    let request = request_result.unwrap();

    // Send the request – should fail with TooManyRedirects
    let send_result = request.send();

    use foundation_core::wire::simple_http::client::HttpClientError;
    if let Err(err) = send_result {
        println!("Actual error: {:?}", err);
        assert!(matches!(err, HttpClientError::TooManyRedirects(_)), "Expected TooManyRedirects error, got: {:?}", err);
    } else {
        panic!("Expected error due to too many redirects, but got Ok");
    }
}
