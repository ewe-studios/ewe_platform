//! Regression tests for issue #861: an HTTPS body from `send_async` ended early.
//!
//! WHY: `send_async` hands the response socket back in blocking mode, the
//! caller (or the method itself) switches it to non-blocking, and the body is
//! then read through `AsyncSendSafeBody`, which parks on readiness. On macOS a
//! `Content-Length` body served over TLS stopped after a few KiB, and
//! `ContentLengthEnforcingIterator` reported `body truncated`. The sync `send`
//! and the `open_exchange` path read the same response in full.
//!
//! WHAT: a local rustls server writes the response head and the body across
//! several TLS records. In one test the final record arrives with the close; in
//! another the server keeps the connection open after the final record. The
//! body must end only when the framing says so: the full `Content-Length`.
//!
//! HOW: `NativeHttpClient::send_async` with the test TLS connector, then the
//! body is drained through `AsyncSendSafeBody`.

#![cfg(all(unix, feature = "ssl-rustls"))]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_core::url::Uri;
use foundation_core::valtron::valtron_test;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::native::connection::Connection;
use foundation_netio::netcap::ssl::rustls::{RustlsAcceptor, RustlsConnector};
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::{PreparedRequest, SystemDnsResolver};
use foundation_netio::shared::http::{
    Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use rustls::pki_types::pem::PemObject;
use zeroize::Zeroizing;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/tcp_and_tls/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).expect("read TLS fixture")
}

fn client_with_test_connector() -> NativeHttpClient<SystemDnsResolver> {
    let cert = fixture("cert.pem");
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from_pem_slice(&cert).expect("cert der"))
        .expect("trust fixture");
    let client_tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    NativeHttpClient::<SystemDnsResolver>::from_system()
        .with_tls_connector(RustlsConnector::with_config(Arc::new(client_tls)))
}

fn get_request(url: &str) -> PreparedRequest {
    let uri = Uri::parse(url).expect("uri");
    let host = match uri.port() {
        Some(p) => format!("localhost:{p}"),
        None => "localhost".to_string(),
    };
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec![host]);
    PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// The body: large enough that the batch reader needs several reads.
fn payload() -> Vec<u8> {
    (0..48_000_u32).map(|i| (i % 251) as u8).collect()
}

/// Serve one request over rustls, writing the head and each body piece as its
/// own TLS record. When `close_with_final` is set, the connection closes as
/// soon as the final record is written; otherwise it stays open for a while.
/// `key_updates` TLS 1.3 `KeyUpdate` messages are sent between the pieces; each
/// makes the socket readable without carrying application data.
fn serve_tls_with_key_updates(pieces: Vec<Vec<u8>>, key_updates: usize) -> u16 {
    let acceptor =
        RustlsAcceptor::from_pem(fixture("cert.pem"), Zeroizing::new(fixture("key.pem")))
            .expect("acceptor");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        let mut tls = acceptor
            .accept(Connection::Tcp(socket))
            .expect("tls accept");
        let mut buf = [0u8; 8192];
        let mut request = Vec::new();
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => request.extend_from_slice(&buf[..n]),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(err) => panic!("server read: {err}"),
            }
        }
        for (index, piece) in pieces.iter().enumerate() {
            tls.write_all(piece).expect("server write");
            tls.flush().expect("server flush");
            if index + 1 < pieces.len() {
                for _ in 0..key_updates {
                    tls.refresh_traffic_keys().expect("queue key update");
                    tls.flush().expect("flush key update");
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        thread::sleep(Duration::from_millis(200));
    });
    port
}

fn serve_tls(pieces: Vec<Vec<u8>>, close_with_final: bool) -> u16 {
    let acceptor =
        RustlsAcceptor::from_pem(fixture("cert.pem"), Zeroizing::new(fixture("key.pem")))
            .expect("acceptor");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        let mut tls = acceptor
            .accept(Connection::Tcp(socket))
            .expect("tls accept");
        let mut buf = [0u8; 8192];
        // Read the request head.
        let mut request = Vec::new();
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => request.extend_from_slice(&buf[..n]),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(err) => panic!("server read: {err}"),
            }
        }
        for (index, piece) in pieces.iter().enumerate() {
            tls.write_all(piece).expect("server write");
            tls.flush().expect("server flush");
            if index + 1 < pieces.len() {
                // Let the client consume this record and park for the next.
                thread::sleep(Duration::from_millis(25));
            }
        }
        if !close_with_final {
            thread::sleep(Duration::from_secs(2));
        }
    });
    port
}

async fn read_async_body(mut body: AsyncSendSafeBody) -> (Vec<u8>, Option<String>) {
    use core::future::poll_fn;
    use core::pin::Pin;
    use futures_core::Stream as FuturesStream;

    let mut all = Vec::new();
    loop {
        match poll_fn(|cx| Pin::new(&mut body).poll_next(cx)).await {
            Some(Ok(chunk)) => all.extend(chunk),
            Some(Err(err)) => return (all, Some(err.to_string())),
            None => return (all, None),
        }
    }
}

fn framed_pieces(body: &[u8], head: &[u8]) -> Vec<Vec<u8>> {
    // The head shares the first record with the first third of the body. The
    // rest crosses two more records.
    let third = body.len() / 3;
    let mut first = head.to_vec();
    first.extend_from_slice(&body[..third]);
    vec![
        first,
        body[third..2 * third].to_vec(),
        body[2 * third..].to_vec(),
    ]
}

#[valtron_test]
async fn send_async_body_survives_tls_control_records_without_app_data() {
    // A TLS 1.3 KeyUpdate is a readable edge with no application data. The
    // async body parks on it, reads nothing, and waits again. On a WAN a TLS
    // record also arrives over several TCP segments, so the first wakes of a
    // long record look the same. The body must survive those wakes and end at
    // the Content-Length, not at the retry budget the blocking reader uses.
    let body = payload();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
        body.len()
    );
    let port = serve_tls_with_key_updates(framed_pieces(&body, head.as_bytes()), 4);
    let url = format!("https://localhost:{port}/control");
    let client = client_with_test_connector();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (status, _headers, async_body) = response.into_async_parts();
    assert_eq!(status, Status::OK);

    let (read, error) = read_async_body(async_body).await;
    assert_eq!(error, None, "body read failed after {} bytes", read.len());
    assert_eq!(
        read.len(),
        body.len(),
        "the body ended early: expected {}, got {}",
        body.len(),
        read.len()
    );
    assert_eq!(read, body);
}

#[valtron_test]
async fn a_tls_close_before_the_content_length_is_an_error_not_a_short_body() {
    // A fix for the early end must not turn a real truncation into a short
    // success: the body ends only when the framing says so (issue #861).
    let full = payload();
    let partial = &full[..8_000];
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        full.len()
    );
    let mut first = head.into_bytes();
    first.extend_from_slice(partial);
    let port = serve_tls(vec![first], true);
    let url = format!("https://localhost:{port}/truncated");
    let client = client_with_test_connector();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (status, _headers, async_body) = response.into_async_parts();
    assert_eq!(status, Status::OK);

    let (read, error) = read_async_body(async_body).await;
    let error = error.expect("a body closed before Content-Length must report an error");
    assert!(
        read.len() < full.len(),
        "the test server sent only {} bytes",
        partial.len()
    );
    assert!(
        error.contains("truncated") || error.contains("EOF") || error.contains("eof"),
        "the error should name the early end, got: {error}"
    );
}

#[valtron_test]
async fn send_async_reads_a_tls_body_split_across_records_that_closes() {
    let body = payload();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let port = serve_tls(framed_pieces(&body, head.as_bytes()), true);
    let url = format!("https://localhost:{port}/split");
    let client = client_with_test_connector();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (status, headers, async_body) = response.into_async_parts();
    assert_eq!(status, Status::OK);
    assert_eq!(
        headers
            .get(&SimpleHeader::CONTENT_LENGTH)
            .and_then(|values| values.first())
            .map(String::as_str),
        Some(body.len().to_string().as_str())
    );

    let (read, error) = read_async_body(async_body).await;
    assert_eq!(error, None, "body read failed after {} bytes", read.len());
    assert_eq!(
        read.len(),
        body.len(),
        "the body ended early: expected {}, got {}",
        body.len(),
        read.len()
    );
    assert_eq!(read, body);
}

#[valtron_test]
async fn send_async_reads_a_tls_body_split_across_records_kept_open() {
    let body = payload();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
        body.len()
    );
    let port = serve_tls(framed_pieces(&body, head.as_bytes()), false);
    let url = format!("https://localhost:{port}/split-open");
    let client = client_with_test_connector();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (status, _headers, async_body) = response.into_async_parts();
    assert_eq!(status, Status::OK);

    let (read, error) = read_async_body(async_body).await;
    assert_eq!(error, None, "body read failed after {} bytes", read.len());
    assert_eq!(read, body);
}
