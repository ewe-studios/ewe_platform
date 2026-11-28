//! A non-2xx response that carries `Connection: close` must finish its body
//! read and let the caller continue.
//!
//! WHY: the AWS KMS local test against moto hangs after moto answers `400` to
//! `TrentService.Decrypt`. The error response has a `Content-Length` body and
//! `Connection: close`. This reproduces that shape on a local TCP server, so it
//! runs without Docker, and fails if the async error-body read never ends.
//!
//! HOW: a raw-TCP server answers with an optional `100 Continue` interim, then
//! a final `400` head and body, then closes. The client reads the body with the
//! same `collect_bytes_bounded` helper that the auth error paths use.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};

use foundation_core::url::Uri;
use foundation_core::valtron::valtron_test;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::body_reader::collect_bytes_bounded;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::{PreparedRequest, SystemDnsResolver};
use foundation_netio::shared::http::{
    Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};

/// The error body that moto sends for a refused `Decrypt`.
const ERROR_BODY: &[u8] = b"{\"__type\":\"AccessDeniedException\"}\n";

/// One response the server writes before it closes.
#[derive(Clone, Copy)]
struct Reply<'a> {
    /// Write a `100 Continue` interim before the final response.
    interim: bool,
    /// Status line of the final response.
    status: &'a str,
    /// Response body.
    body: &'a [u8],
    /// Write the head and the body in separate writes, with a small pause.
    split_body: bool,
}

/// Serve exactly one connection, then close it.
fn serve(reply: Reply<'static>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        if reply.interim {
            stream
                .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
                .expect("write interim");
            stream.flush().expect("flush interim");
        }
        let head = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
            reply.status,
            reply.body.len()
        );
        stream.write_all(head.as_bytes()).expect("write head");
        stream.flush().expect("flush head");
        if reply.split_body {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        stream.write_all(reply.body).expect("write body");
        stream.flush().expect("flush body");
    });
    addr
}

fn post_request(url: &str) -> PreparedRequest {
    let uri = Uri::parse(url).expect("uri");
    let host = match uri.port() {
        Some(port) => format!(
            "{}:{port}",
            uri.host_str().unwrap_or_else(|| "127.0.0.1".to_string())
        ),
        None => uri.host_str().unwrap_or_else(|| "127.0.0.1".to_string()),
    };
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec![host]);
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["application/x-amz-json-1.1".to_string()],
    );
    PreparedRequest {
        method: SimpleMethod::POST,
        url: uri,
        headers,
        body: SendSafeBody::Text("{\"KeyId\":\"k\"}".to_string()),
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// Send a request, assert the final status, and read the whole error body.
async fn read_error_body(reply: Reply<'static>) -> Vec<u8> {
    let addr = serve(reply);
    let url = format!("http://127.0.0.1:{}/", addr.port());
    let client = NativeHttpClient::<SystemDnsResolver>::from_system();
    let response = client
        .send_async(post_request(&url))
        .await
        .expect("send the request");
    assert_eq!(response.get_status(), Status::BadRequest);
    let (_status, _headers, body) = response.into_async_parts();
    collect_bytes_bounded(body, 64 * 1024).await
}

#[valtron_test(timeout = 10_000)]
async fn a_400_body_with_connection_close_is_read() {
    let body = read_error_body(Reply {
        interim: false,
        status: "400 Bad Request",
        body: ERROR_BODY,
        split_body: false,
    })
    .await;
    assert_eq!(body, ERROR_BODY);
}

#[valtron_test(timeout = 10_000)]
async fn a_400_body_after_an_interim_is_read() {
    let body = read_error_body(Reply {
        interim: true,
        status: "400 Bad Request",
        body: ERROR_BODY,
        split_body: false,
    })
    .await;
    assert_eq!(body, ERROR_BODY);
}

#[valtron_test(timeout = 10_000)]
async fn a_400_body_that_arrives_after_the_head_is_read() {
    let body = read_error_body(Reply {
        interim: true,
        status: "400 Bad Request",
        body: ERROR_BODY,
        split_body: true,
    })
    .await;
    assert_eq!(body, ERROR_BODY);
}
