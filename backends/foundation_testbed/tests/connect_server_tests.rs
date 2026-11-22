#![cfg(feature = "wasm")]
//! The `/connect` route on the testbed server (spec 88 feature 03, T4).
//!
//! WHY: the transport cases need a real `ConnectRPC` endpoint on the same
//! origin as the suite. This checks the route itself, without a browser or a
//! wasm module.
//! WHAT: a unary call returns the handler's message, a handler error keeps its
//! code, and a server stream writes its frames in order.
//! HOW: a raw loopback connection against the real `TestServer`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use foundation_testbed::wasm::server::{ServerLimits, start_serving_with_echo};

/// Connect to the server, retrying until its thread binds the port.
///
/// The wait is bounded, so a server that never binds fails the test instead of
/// hanging it (rule R4).
fn connect(port: u16) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => return stream,
            Err(e) if Instant::now() >= deadline => {
                panic!("connect 127.0.0.1:{port}: {e}");
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// The status line, the headers and the body of one HTTP/1.1 response.
struct Answer {
    status: String,
    headers: String,
    body: Vec<u8>,
}

/// Send one HTTP/1.1 POST and split the response into its parts.
fn post(port: u16, path: &str, content_type: &str, body: &[u8]) -> Answer {
    let mut stream = connect(port);
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write the request head");
    stream.write_all(body).expect("write the request body");
    stream.flush().expect("flush the request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the response");

    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("the response has no head: {raw:?}"));
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut lines = head.split("\r\n");
    let status = lines.next().unwrap_or_default().to_string();
    let headers = lines.collect::<Vec<_>>().join("\n");
    Answer {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    }
}

fn init_runtime() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "foundation_http=debug,foundation_connectrpc=debug",
        ))
        .try_init();
    foundation_browser::runtime::ensure_runtime(None);
}

/// The `content-length` header of an answer, when it is present.
fn content_length(answer: &Answer) -> Option<usize> {
    for line in answer.headers.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse().ok();
        }
    }
    None
}

/// A Connect streaming request envelope around one message.
fn enveloped(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + message.len());
    out.push(0);
    out.extend_from_slice(
        &u32::try_from(message.len())
            .expect("the message fits in u32")
            .to_be_bytes(),
    );
    out.extend_from_slice(message);
    out
}

#[test]
fn connect_unary_answers_with_the_echoed_message() {
    init_runtime();
    let dir = tempfile::tempdir().expect("temp dir");
    let server = start_serving_with_echo(dir.path(), &ServerLimits::default()).expect("start");
    let port = server.port();

    let answer = post(
        port,
        "/connect/case.CaseService/Unary",
        "application/proto",
        b"hello",
    );
    server.shutdown();

    assert!(
        answer.status.starts_with("HTTP/1.1 200"),
        "the unary route answers 200: {}",
        answer.status
    );
    assert!(
        answer.headers.contains("application/proto"),
        "the unary route keeps the Connect content type: {}",
        answer.headers
    );
    assert_eq!(
        content_length(&answer),
        Some(answer.body.len()),
        "a keep-alive response declares its body length: {}",
        answer.headers
    );
    assert_eq!(
        answer.body,
        b"echo:hello".to_vec(),
        "the handler's message reaches the client"
    );
}

#[test]
fn connect_handler_error_carries_its_code() {
    init_runtime();
    let dir = tempfile::tempdir().expect("temp dir");
    let server = start_serving_with_echo(dir.path(), &ServerLimits::default()).expect("start");
    let port = server.port();

    let answer = post(
        port,
        "/connect/case.CaseService/Fail",
        "application/proto",
        b"hello",
    );
    server.shutdown();

    assert!(
        answer.status.starts_with("HTTP/1.1 400"),
        "invalid_argument answers 400: {}",
        answer.status
    );
    let wire: serde_json::Value = serde_json::from_slice(&answer.body).expect("a JSON error body");
    assert_eq!(
        wire.get("code").and_then(serde_json::Value::as_str),
        Some("invalid_argument"),
        "the error keeps its code: {wire}"
    );
    assert_eq!(
        content_length(&answer),
        Some(answer.body.len()),
        "an error response declares its body length: {}",
        answer.headers
    );
}

#[test]
fn connect_server_stream_writes_every_frame_in_order() {
    init_runtime();
    let dir = tempfile::tempdir().expect("temp dir");
    let server = start_serving_with_echo(dir.path(), &ServerLimits::default()).expect("start");
    let port = server.port();

    let answer = post(
        port,
        "/connect/case.CaseService/Stream",
        "application/connect+proto",
        &enveloped(b"seed"),
    );
    server.shutdown();

    assert!(
        answer.status.starts_with("HTTP/1.1 200"),
        "the stream route answers 200: {}",
        answer.status
    );
    assert_eq!(
        content_length(&answer),
        Some(answer.body.len()),
        "a collected stream declares its body length: {}",
        answer.headers
    );
    let mut body = answer.body.as_slice();
    let mut messages = Vec::new();
    while body.len() >= 5 {
        let flags = body[0];
        let len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        let payload = body[5..5 + len].to_vec();
        if flags == 0x00 {
            messages.push(payload);
        }
        body = &body[5 + len..];
    }
    assert_eq!(
        messages,
        vec![
            b"frame-0".to_vec(),
            b"frame-1".to_vec(),
            b"frame-2".to_vec()
        ],
        "every frame arrives, in order"
    );
}
