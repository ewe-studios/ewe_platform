#![cfg(feature = "wasm")]
//! The local endpoint every host offers (spec 88 invariant I5, scenario S4).
//!
//! WHY: a suite needs an endpoint that answers before it runs; this checks the
//! route itself, without a browser or a wasm module.
//! WHAT: a POST with a body answers 200 and returns the method, the path, the
//! headers and the body it received; a body over the limit answers 413.
//! HOW: a raw loopback connection against the real `TestServer`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use foundation_testbed::wasm::server::{ServerLimits, start_serving_with_echo};

/// Connect to the server, retrying until its thread binds the port.
///
/// The server thread starts after `start_serving_with_echo` returns. The wait is
/// bounded, so a server that never binds fails the test instead of hanging it.
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

/// Send one HTTP/1.1 request and return the whole response as text.
fn request(port: u16, method: &str, path: &str, headers: &str, body: &str) -> String {
    let mut stream = connect(port);
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write the request");
    stream.flush().expect("flush the request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read the response");
    response
}

fn init_trace() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("foundation_http=debug"))
        .try_init();
}

#[test]
fn echo_answers_a_post_with_the_request_it_received() {
    init_trace();
    foundation_browser::runtime::ensure_runtime(None);
    let dir = tempfile::tempdir().expect("temp dir");
    let server = start_serving_with_echo(dir.path(), &ServerLimits::default()).expect("start");
    let port = server.port();

    let response = request(
        port,
        "POST",
        "/echo",
        "x-fwt-probe: server-test\r\n",
        "hello",
    );
    server.shutdown();

    assert!(
        response.starts_with("HTTP/1.1 200"),
        "the route answers 200: {response}"
    );
    assert!(
        response.contains("\"method\":\"POST\""),
        "the method is echoed: {response}"
    );
    assert!(
        response.contains("\"path\":\"/echo\""),
        "the path is echoed: {response}"
    );
    assert!(
        response.contains("\"x-fwt-probe\":\"server-test\""),
        "the headers are echoed: {response}"
    );
    assert!(
        response.contains("\"body\":\"aGVsbG8=\""),
        "the body is echoed as base64: {response}"
    );
    assert!(
        response.contains("\"bytes\":5"),
        "the body length is echoed: {response}"
    );
}

#[test]
fn echo_rejects_a_body_over_the_limit() {
    foundation_browser::runtime::ensure_runtime(None);
    let dir = tempfile::tempdir().expect("temp dir");
    let server =
        start_serving_with_echo(dir.path(), &ServerLimits::default().with_max_echo_body(4))
            .expect("start");
    let port = server.port();

    let response = request(port, "POST", "/echo", "", "hello");
    server.shutdown();

    assert!(
        response.starts_with("HTTP/1.1 413"),
        "a body over the limit answers 413: {response}"
    );
}
