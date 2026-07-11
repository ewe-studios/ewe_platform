//! Tests for F51 (unified network client) — Stage 1–6 deliverables.
//!
//! WHY: F51 restructured the HTTP client surface (dissolved `SimpleHttpClient`,
//! added `HttpExchangeClientTask`, `HttpClientBuilder`, `open_exchange`,
//! `WebSocketConnector`). These tests verify the new surface works correctly.
//!
//! WHAT: Tests for `NativeHttpClient` construction/config, `HttpClientBuilder`,
//! `open_exchange` → `HttpExchangeClientTask` iteration,
//! `WebSocketConnector::open_websocket`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{self, valtron_test, Stream, TaskIterator, TaskIteratorExt};
use foundation_netio::http::NativeHttpClient;
use foundation_netio::http_client_builder::HttpClientBuilder;
use foundation_netio::simple_http::client::shared::http_client::HttpClient;
use foundation_netio::simple_http::client::shared::request_task::{
    HttpExchange, HttpExchangeClientTask, HttpExchangePending,
};
use foundation_netio::simple_http::client::shared::{
    ClientConfig, PreparedRequest, SystemDnsResolver,
};
use foundation_netio::simple_http::shared::{
    Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use foundation_netio::websocket::shared::connector::WebSocketConnector;
use foundation_netio::websocket::shared::handshake::compute_accept_key;

// ============================================================================
// Helpers
// ============================================================================

fn system_client() -> NativeHttpClient<SystemDnsResolver> {
    NativeHttpClient::from_system()
}

fn get_request(url: &str) -> PreparedRequest {
    let uri = foundation_core::url::Uri::parse(url).expect("uri");
    let host = match uri.port() {
        Some(p) => format!(
            "{}:{p}",
            uri.host_str().unwrap_or_else(|| "127.0.0.1".to_string())
        ),
        None => uri.host_str().unwrap_or_else(|| "127.0.0.1".to_string()),
    };
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec![host]);
    PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    }
}

/// Start a tiny raw-TCP HTTP server on an ephemeral port.
/// Returns the listening address. The server responds once then shuts down.
fn raw_http_serve(status: u16, body: &'static str) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let body_bytes = body.as_bytes().to_vec();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept");
            let mut buf = [0u8; 4096];
            let _n = stream.read(&mut buf).unwrap_or(0);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body_bytes.len()
            );
            let mut out = response.into_bytes();
            out.extend_from_slice(&body_bytes);
            let _ = stream.write_all(&out);
            break;
        }
    });
    addr
}

// ============================================================================
// NativeHttpClient construction and config (Stage 2)
// ============================================================================

#[test]
fn native_http_client_from_system() {
    let client = system_client();
    let config = client.client_config();
    assert_eq!(config.max_redirects, 5);
}

#[test]
fn native_http_client_default_has_pool() {
    let client = NativeHttpClient::<SystemDnsResolver>::default();
    let config = client.client_config();
    assert_eq!(config.max_redirects, 5);
    assert!(client.client_pool().is_some());
}

#[test]
fn native_http_client_config() {
    let custom = ClientConfig {
        max_redirects: 10,
        ..ClientConfig::default()
    };
    let client = system_client().config(custom);
    assert_eq!(client.client_config().max_redirects, 10);
}

#[test]
fn native_http_client_max_redirects() {
    let client = system_client().max_redirects(3);
    assert_eq!(client.client_config().max_redirects, 3);
}

#[test]
fn native_http_client_connect_timeout() {
    let client = system_client().connect_timeout(Duration::from_secs(5));
    assert_eq!(
        client
            .client_config()
            .timeout_calculator
            .config()
            .connect_timeout,
        Duration::from_secs(5)
    );
}

#[test]
fn native_http_client_read_timeout() {
    let client = system_client().read_timeout(Duration::from_secs(15));
    assert_eq!(
        client
            .client_config()
            .timeout_calculator
            .config()
            .min_read_timeout,
        Duration::from_secs(15)
    );
}

#[test]
fn native_http_client_builder_chaining() {
    let client = system_client()
        .connect_timeout(Duration::from_secs(10))
        .max_redirects(3)
        .max_body_size(Some(1_048_576));
    let cfg = client.client_config();
    assert_eq!(cfg.max_redirects, 3);
    assert_eq!(cfg.max_body_size, Some(1_048_576));
}

#[test]
fn native_http_client_default() {
    let client = NativeHttpClient::<SystemDnsResolver>::default();
    assert_eq!(client.client_config().max_redirects, 5);
}

// ============================================================================
// HttpClientBuilder (Stage 2b)
// ============================================================================

#[test]
fn http_client_builder_new() {
    let client = HttpClientBuilder::new().build();
    let _ = Arc::as_ptr(&client);
}

#[test]
fn http_client_builder_timeouts() {
    let client = HttpClientBuilder::new()
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(10))
        .write_timeout(Duration::from_secs(20))
        .max_redirects(2)
        .max_retries(3)
        .build();
    let _ = Arc::as_ptr(&client);
}

#[test]
fn http_client_builder_default_headers() {
    let client = HttpClientBuilder::new()
        .default_header("X-Custom", "value1")
        .default_header("Accept", "application/json")
        .build();
    let _ = Arc::as_ptr(&client);
}

#[test]
fn http_client_builder_body_limits() {
    let client = HttpClientBuilder::new()
        .max_body_size(Some(4096))
        .full_body_threshold(1024)
        .batch_size(64)
        .build();
    let _ = Arc::as_ptr(&client);
}

#[test]
fn http_client_builder_proxy() {
    assert!(HttpClientBuilder::new()
        .proxy("http://proxy.example.com:8080")
        .is_ok());
}

#[test]
fn http_client_builder_proxy_invalid() {
    assert!(HttpClientBuilder::new().proxy(" not a url :// ").is_err());
}

#[test]
fn http_client_builder_expect_continue() {
    let client = HttpClientBuilder::new().expect_continue(false).build();
    let _ = Arc::as_ptr(&client);
}

#[valtron_test]
fn http_client_builder_send() {
    let addr = raw_http_serve(200, "hello");
    let url = format!("http://127.0.0.1:{}/test", addr.port());

    let client = HttpClientBuilder::new().max_redirects(0).build();
    let result = client.send(get_request(&url));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get_status(), Status::OK);
}

// ============================================================================
// open_exchange → HttpExchangeClientTask (Stage 1b)
// ============================================================================

#[valtron_test]
fn open_exchange_yields_head_then_body() {
    let addr = raw_http_serve(200, "hello world");
    let url = format!("http://127.0.0.1:{}/test", addr.port());

    let client = Arc::new(system_client().max_redirects(0));
    let task = client.open_exchange(get_request(&url));

    let mut stream = valtron::execute(task, None).expect("execute");

    let mut saw_head = false;
    let mut body_bytes = Vec::new();
    loop {
        match stream.next() {
            Some(Stream::Next(HttpExchange::Head { status, .. })) => {
                assert_eq!(status, Status::OK);
                saw_head = true;
            }
            Some(Stream::Next(HttpExchange::BodyChunk(bytes))) => {
                body_bytes.extend_from_slice(&bytes);
            }
            Some(Stream::Next(HttpExchange::Failed(e))) => {
                panic!("unexpected Failed: {e}");
            }
            Some(_) => continue,
            None => break,
        }
    }
    assert!(saw_head, "should have seen Head");
    assert_eq!(String::from_utf8_lossy(&body_bytes), "hello world");
}

#[valtron_test]
fn open_exchange_reports_failure() {
    let client = Arc::new(
        system_client()
            .max_redirects(0)
            .connect_timeout(Duration::from_millis(200)),
    );
    let task = client.open_exchange(get_request("http://127.0.0.1:1/nothing"));

    let mut stream = valtron::execute(task, None).expect("execute");
    let mut saw_failed = false;
    loop {
        match stream.next() {
            Some(Stream::Next(HttpExchange::Failed(_))) => {
                saw_failed = true;
                break;
            }
            Some(Stream::Next(HttpExchange::Head { .. })) => {
                // Should not succeed for dead port.
                break;
            }
            None => break,
            _ => continue,
        }
    }
    assert!(saw_failed, "should have seen Failed for dead server");
}

#[test]
fn http_exchange_client_task_smoke() {
    let req = get_request("http://example.com/");
    let client = Arc::new(system_client().max_redirects(0));
    let mut task = client.open_exchange(req);
    assert!(task.next_status().is_some());
}

// ============================================================================
// WebSocketConnector (Stage 5–6)
// ============================================================================

#[test]
fn websocket_connector_trait_bound() {
    fn _require_ws<T: WebSocketConnector>(_: &T) {}
    _require_ws(&system_client());
}

#[valtron_test]
fn websocket_connector_rejects_plain_http() {
    let addr = raw_http_serve(200, "not a websocket");
    let url = format!("ws://127.0.0.1:{}/ws", addr.port());

    let client = system_client();
    let result = client.open_websocket(&url, SimpleHeaders::new());
    assert!(result.is_err(), "plain HTTP should be rejected");
}

#[valtron_test]
fn websocket_connector_accepts_valid_101() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);

            let key_line = request
                .lines()
                .find(|l| l.to_lowercase().starts_with("sec-websocket-key:"))
                .expect("should have Sec-WebSocket-Key");
            let client_key = key_line
                .split(':')
                .nth(1)
                .map(|s| s.trim())
                .expect("key value");
            let accept = compute_accept_key(client_key);

            let response = format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
                 Upgrade: websocket\r\n\
                 Connection: Upgrade\r\n\
                 Sec-WebSocket-Accept: {accept}\r\n\r\n"
            );
            let _ = stream.write_all(response.as_bytes());
            break;
        }
    });

    let url = format!("ws://127.0.0.1:{}/ws", addr.port());
    let client = system_client();
    let result = client.open_websocket(&url, SimpleHeaders::new());
    assert!(result.is_ok(), "valid 101 handshake should succeed");
}

// ============================================================================
// Compatibility: SimpleHttpClient alias (Stage 2a)
// ============================================================================

#[test]
#[allow(deprecated)]
fn simple_http_client_alias_works() {
    use foundation_netio::http::SimpleHttpClient;
    let client = SimpleHttpClient::<SystemDnsResolver>::default();
    assert_eq!(client.client_config().max_redirects, 5);
}
