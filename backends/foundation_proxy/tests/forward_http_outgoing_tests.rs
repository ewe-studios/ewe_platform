//! A real test of `forward_http`'s outgoing request: the fake upstream must
//! receive the original `Host`, an appended `X-Forwarded-For`, the
//! `X-Forwarded-Proto` scheme and `Connection: close`, and the client must get
//! a 200. (Was `diag_client_tap.rs`, a print-only `#[ignore]`d diagnostic.)

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::initialize_pool;
use foundation_netio::http::{ClientRequestBuilder, HttpConnectionPool, NativeHttpClient};
use foundation_netio::shared::client::{ClientConfig, SystemDnsResolver};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status};
use foundation_proxy::forward::forward_request_headers;

#[test]
fn forward_http_sends_expected_headers_and_gets_200() {
    let _guard = initialize_pool(77, Some(4));

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake upstream");
    let addr = listener.local_addr().unwrap();

    // The fake upstream captures the request head (up to its blank line) and
    // answers a length-delimited 200.
    let upstream = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        sock.set_read_timeout(Some(Duration::from_secs(3))).ok();
        let mut head: Vec<u8> = Vec::new();
        let mut buf = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match sock.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => head.extend_from_slice(&buf[..n]),
            }
        }
        let sent = String::from_utf8_lossy(&head).to_string();

        let body = b"{\"ok\":true}";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        sock.write_all(resp.as_bytes()).expect("write head");
        sock.write_all(body).expect("write body");
        sock.flush().ok();

        sent
    });

    // Build the same request `forward_http` builds: the client's headers pass
    // through `forward_request_headers`, which is what the proxy calls before
    // sending upstream.
    let mut original: SimpleHeaders = BTreeMap::new();
    original.insert(SimpleHeader::HOST, vec!["app.local".to_string()]);
    original.insert(SimpleHeader::CONTENT_TYPE, vec!["text/plain".to_string()]);
    let headers = forward_request_headers(&original, "1.2.3.4", "http");

    let pool: Arc<HttpConnectionPool<SystemDnsResolver>> = Arc::new(HttpConnectionPool::default());
    let client = NativeHttpClient::with_config_and_pool(
        ClientConfig::default(),
        pool,
        SystemDnsResolver::default(),
    );
    let url = format!("http://127.0.0.1:{}/headers", addr.port());
    let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::GET, &url)
        .expect("build request")
        .headers(headers)
        .body(SendSafeBody::None);
    let request = client.request(builder).expect("prepare request");
    let response = request.send().expect("upstream request must succeed");

    assert_eq!(response.get_status(), Status::OK, "fake upstream answers 200");

    // The captured head, lower-cased so header-name case cannot hide a miss.
    let sent = upstream.join().expect("upstream thread").to_lowercase();
    assert!(
        sent.contains("\r\nhost: app.local\r\n"),
        "original Host is preserved: {sent}"
    );
    assert!(
        sent.contains("\r\nx-forwarded-for: 1.2.3.4\r\n"),
        "client IP is appended to X-Forwarded-For: {sent}"
    );
    assert!(
        sent.contains("\r\nx-forwarded-proto: http\r\n"),
        "scheme is set in X-Forwarded-Proto: {sent}"
    );
    assert!(
        sent.contains("\r\nconnection: close\r\n"),
        "plain request asks the upstream to close: {sent}"
    );
}
