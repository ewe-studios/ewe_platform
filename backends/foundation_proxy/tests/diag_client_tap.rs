//! TEMPORARY diagnostic: capture the exact bytes foundation_netio's client
//! sends, and see whether it parses a canned httpbin-style response.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use std::collections::BTreeMap;

use foundation_core::valtron::initialize_pool;
use foundation_netio::simple_http::client::native::{
    ClientRequestBuilder, HttpConnectionPool,
};
use foundation_netio::simple_http::client::shared::{ClientConfig, SystemDnsResolver};
use foundation_netio::simple_http::client::NativeHttpClient;
use foundation_netio::simple_http::shared::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};
use std::sync::Arc;

#[test]
#[ignore = "diagnostic"]
fn diag_tap() {
    let _pool = initialize_pool(77, Some(4));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(3))).ok();
        let mut buf = [0u8; 4096];
        let n = sock.read(&mut buf).unwrap_or(0);
        eprintln!("=== CLIENT SENT ({n} bytes) ===\n{}", String::from_utf8_lossy(&buf[..n]));
        eprintln!("=== END CLIENT REQUEST ===");
        // Canned httpbin-style response: keep-alive + Content-Length.
        let body = b"{\"headers\":{}}";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nServer: gunicorn/19.9.0\r\nConnection: keep-alive\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
            body.len()
        );
        sock.write_all(resp.as_bytes()).unwrap();
        sock.write_all(body).unwrap();
        sock.flush().unwrap();
        // Keep the socket open (mimic keep-alive) briefly.
        std::thread::sleep(Duration::from_secs(2));
    });

    let pool: Arc<HttpConnectionPool<SystemDnsResolver>> = Arc::new(HttpConnectionPool::default());
    let client = NativeHttpClient::with_config_and_pool(
        ClientConfig::default(),
        pool,
        SystemDnsResolver::default(),
    );
    let url = format!("http://127.0.0.1:{}/headers", addr.port());

    // Mimic exactly what forward_http builds: a replaced header map with the
    // ORIGINAL Host plus X-Forwarded-* and Connection: close.
    let mut headers: SimpleHeaders = BTreeMap::new();
    headers.insert(SimpleHeader::HOST, vec!["app.local".to_string()]);
    headers.insert(SimpleHeader::custom("x-forwarded-for"), vec!["1.2.3.4".to_string()]);
    headers.insert(SimpleHeader::custom("x-forwarded-proto"), vec!["http".to_string()]);
    headers.insert(SimpleHeader::custom("x-forwarded-host"), vec!["app.local".to_string()]);
    headers.insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);

    let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::GET, &url)
        .unwrap()
        .headers(headers)
        .body(SendSafeBody::None);
    let req = client.request(builder).unwrap();
    match req.send() {
        Ok(resp) => eprintln!("=== SEND OK status={:?} ===", resp.get_status()),
        Err(e) => eprintln!("=== SEND ERR: {e} ==="),
    }
    handle.join().ok();
}
