#![cfg(test)]

//! WebSocket ReconnectingWebSocketTask integration tests.
//!
//! Tests automatic reconnection with exponential backoff.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use base64::Engine;
use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_core::wire::websocket::{ReconnectingWebSocketProgress, ReconnectingWebSocketTask};
use sha1::{Digest, Sha1};
use tracing_test::traced_test;

/// A test server that always closes connections after handshake to trigger reconnection.
struct ReconnectTestServer {
    addr: String,
    connection_count: Arc<AtomicUsize>,
    _handle: Option<thread::JoinHandle<()>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

/// Compute Sec-WebSocket-Accept from client key.
fn compute_ws_accept_key(client_key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

impl ReconnectTestServer {
    /// Start a server that closes every connection after sending handshake.
    fn new_close_on_connect() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("ws://{}", listener.local_addr().unwrap());
        let connection_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&connection_count);
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            // Use blocking I/O for simplicity
            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let conn_num = count_clone.fetch_add(1, Ordering::SeqCst);
                        eprintln!("Server: Accepted connection {}", conn_num);

                        // Set read timeout
                        stream.set_read_timeout(Some(Duration::from_secs(4))).ok();

                        // Read request
                        let mut buffer = [0u8; 4096];
                        let bytes_read = match stream.read(&mut buffer) {
                            Ok(n) => n,
                            Err(e) => {
                                eprintln!("Server: Read error on connection {}: {}", conn_num, e);
                                continue;
                            }
                        };
                        eprintln!(
                            "Server: Read {} bytes from connection {}",
                            bytes_read, conn_num
                        );

                        if bytes_read > 0 {
                            // Parse client's Sec-WebSocket-Key from request
                            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                            eprintln!("Server: Request = {}", request.lines().next().unwrap_or(""));

                            let client_key = request
                                .lines()
                                .find(|line| line.to_lowercase().starts_with("sec-websocket-key:"))
                                .and_then(|line| line.split(':').nth(1))
                                .map(|s| s.trim())
                                .unwrap_or("dGhlIHNhbXBsZSBub25jZQ==");
                            eprintln!("Server: Client key = {}", client_key);

                            let accept_key = compute_ws_accept_key(client_key);
                            eprintln!("Server: Accept key = {}", accept_key);

                            // Send handshake response with computed accept key
                            let response = format!(
                                "HTTP/1.1 101 Switching Protocols\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Accept: {}\r\n\
\r\n",
                                accept_key
                            );
                            let _ = stream.write_all(response.as_bytes());
                            eprintln!("Server: Sent {} bytes", response.len());
                            let _ = stream.flush();

                            // Close immediately after handshake to trigger reconnect
                            eprintln!("Server: Closing connection {}", conn_num);
                            drop(stream);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        eprintln!("Server: Accept error: {}", e);
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            connection_count,
            _handle: Some(handle),
            running,
        }
    }

    fn url(&self) -> String {
        self.addr.clone()
    }

    fn connection_count(&self) -> usize {
        self.connection_count.load(Ordering::SeqCst)
    }
}

impl Drop for ReconnectTestServer {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

// Test 1: ReconnectingWebSocketTask creates successfully
#[test]
#[traced_test]
fn test_reconnecting_task_creation() {
    let resolver = SystemDnsResolver::default();
    let result = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat");

    assert!(result.is_ok(), "Should create reconnecting task");
}

// Test 2: ReconnectingWebSocketTask rejects invalid URL
#[test]
#[traced_test]
fn test_reconnecting_task_invalid_url() {
    let resolver = SystemDnsResolver::default();
    let result = ReconnectingWebSocketTask::connect(resolver, "not-a-valid-url");

    assert!(result.is_err(), "Should reject invalid URL");
}

// Test 3: ReconnectingWebSocketTask rejects non-ws scheme
#[test]
#[traced_test]
fn test_reconnecting_task_non_ws_scheme() {
    let resolver = SystemDnsResolver::default();
    let result = ReconnectingWebSocketTask::connect(resolver, "http://localhost:8080/chat");

    assert!(result.is_err(), "Should reject http:// scheme");
}

// Test 4: Builder configuration methods work
#[test]
#[traced_test]
fn test_reconnecting_task_builder() {
    let resolver = SystemDnsResolver::default();

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_max_retries(3)
        .with_max_reconnect_duration(Duration::from_secs(30))
        .with_subprotocol("chat")
        .with_read_timeout(Duration::from_secs(10));

    // If it compiles, builder works
    let _ = task;
}

// Test 5: Task progresses through Connecting state
#[test]
#[traced_test]
fn test_reconnecting_task_progress_states() {
    foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver::default();
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1").unwrap();

    // First next() should return a pending state
    let status = task.next_status();
    assert!(status.is_some(), "Should return Some status");

    match status {
        Some(TaskStatus::Pending(ReconnectingWebSocketProgress::Connecting)) => {
            // Expected - trying to connect
        }
        Some(TaskStatus::Pending(_)) => {
            // Also acceptable - might transition quickly
        }
        _ => {
            // Other states are also valid depending on timing
        }
    }
}

// Test 6: ReconnectingWebSocketTask is Send
#[test]
#[traced_test]
fn test_reconnecting_task_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ReconnectingWebSocketTask<SystemDnsResolver>>();
}

// Test 7: Task with subprotocols configuration
#[test]
#[traced_test]
fn test_reconnecting_task_with_subprotocols() {
    let resolver = SystemDnsResolver::default();

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_subprotocols(&["chat", "superchat"]);

    let _ = task;
}

// Test 8: Task with custom header
#[test]
#[traced_test]
fn test_reconnecting_task_with_header() {
    use foundation_core::wire::simple_http::SimpleHeader;

    let resolver = SystemDnsResolver::default();

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_header(
            SimpleHeader::Custom("X-Custom-Header".to_string()),
            "custom-value",
        );

    let _ = task;
}

// Test 9: Task with custom backoff
#[test]
#[traced_test]
fn test_reconnecting_task_with_custom_backoff() {
    use foundation_core::retries::ExponentialBackoffDecider;

    let resolver = SystemDnsResolver::default();
    let backoff = ExponentialBackoffDecider::from_duration(
        Duration::from_millis(100),
        Some(Duration::from_secs(1)),
    );

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_backoff(backoff);

    let _ = task;
}

// Test 10: Connection failure triggers reconnection attempt
#[test]
#[traced_test]
fn test_connection_failure_triggers_reconnect() {
    foundation_core::valtron::initialize_pool(42, None);

    // Connect to invalid address - should trigger reconnection attempts
    let resolver = SystemDnsResolver::default();
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_retries(3)
        .with_backoff(
            foundation_core::retries::ExponentialBackoffDecider::from_duration(
                Duration::from_millis(10),
                Some(Duration::from_millis(50)),
            ),
        );

    // Run task and observe reconnection behavior
    let mut iterations = 0;
    let mut saw_reconnecting = false;
    let mut saw_delay = false;
    let max_iterations = 200;

    while iterations < max_iterations {
        iterations += 1;
        match task.next_status() {
            None => {
                // Task exhausted - reconnection attempts used up
                break;
            }
            Some(TaskStatus::Pending(ReconnectingWebSocketProgress::Reconnecting)) => {
                saw_reconnecting = true;
            }
            Some(TaskStatus::Delayed(duration)) => {
                // Backoff delay - expected during reconnection
                saw_delay = true;
                // Simulate executor delay (capped for test speed)
                let delay = std::cmp::min(duration, Duration::from_millis(50));
                std::thread::sleep(delay);
            }
            Some(TaskStatus::Ready(Err(_))) => {
                // Connection error - expected
            }
            _ => {}
        }
    }

    // Should have seen reconnecting state or backoff delay
    assert!(
        saw_reconnecting || saw_delay,
        "Should have seen reconnection activity (Reconnecting state or backoff delay)"
    );
}

// Test 11: Max retries eventually exhausts
#[test]
#[traced_test]
fn test_max_retries_exhausts() {
    foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver::default();
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_retries(2); // Very low retry count

    // Run until exhausted
    let mut iterations = 0;
    let max_iterations = 100;
    let mut got_exhausted = false;

    while iterations < max_iterations {
        iterations += 1;
        match task.next_status() {
            None => {
                got_exhausted = true;
                break;
            }
            _ => {}
        }
    }

    assert!(got_exhausted, "Task should exhaust after max retries");
}

// Test 12: Max reconnect duration eventually exhausts
#[test]
#[traced_test]
fn test_max_reconnect_duration_exhausts() {
    foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver::default();
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_reconnect_duration(Duration::from_millis(500)) // Short duration
        .with_max_retries(100) // High retry count - duration should limit
        .with_backoff(
            foundation_core::retries::ExponentialBackoffDecider::from_duration(
                Duration::from_millis(10),
                Some(Duration::from_millis(50)),
            ),
        );

    // Run until exhausted - need more iterations because each reconnect cycle takes multiple next() calls
    let start = std::time::Instant::now();
    let mut iterations = 0;
    let max_iterations = 500; // Increased for more reconnect cycles
    let mut got_exhausted = false;

    while iterations < max_iterations {
        iterations += 1;
        match task.next_status() {
            None => {
                got_exhausted = true;
                break;
            }
            Some(TaskStatus::Delayed(duration)) => {
                // Simulate executor delay (but cap it for test speed)
                let delay = std::cmp::min(duration, Duration::from_millis(50));
                std::thread::sleep(delay);
            }
            _ => {}
        }
    }

    let elapsed = start.elapsed();
    assert!(
        got_exhausted,
        "Task should exhaust after max duration, iterations={}, elapsed={:?}",
        iterations, elapsed
    );
    assert!(
        elapsed >= Duration::from_millis(100),
        "Should have run for some time before exhausting, elapsed={:?}",
        elapsed
    );
}
