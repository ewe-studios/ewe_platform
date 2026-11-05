//! `TestHttpServer` tests moved out of `src/http/server.rs` (spec 70
//! feature 59, rule 7). Every item the tests use is public.

use std::io::Write as _;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use foundation_netio::shared::http::SendSafeBody;
use foundation_testing::http::{HttpResponse, TestHttpServer};

/// WHY: Verify TestHttpServer can start and provides valid URL
/// WHAT: Tests basic construction and URL generation
#[test]
fn test_server_start() {
    let server = TestHttpServer::start();
    assert!(server.base_url().starts_with("http://127.0.0.1:"));
    assert!(server.url("/test").ends_with("/test"));
}

/// WHY: Verify custom response handler works
/// WHAT: Tests that custom handler is called
#[test]
fn test_custom_response() {
    let server = TestHttpServer::with_response(|req| {
        if req.path.url.as_str() == "/redirect" {
            HttpResponse::redirect("/target")
        } else {
            HttpResponse::status(201, "Created")
        }
    });

    // Just verify server starts (actual HTTP testing requires HTTP client)
    assert!(server.base_url().starts_with("http://"));
}

/// WHY: Verify HttpResponse::ok creates proper response
/// WHAT: Tests response construction
#[test]
fn test_http_response_ok() {
    let response = HttpResponse::ok(b"test");
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"test");
}

/// WHY: Verify HttpResponse::redirect creates proper response
/// WHAT: Tests redirect response construction
#[test]
fn test_http_response_redirect() {
    let response = HttpResponse::redirect("/new-location");
    assert_eq!(response.status, 302);
    assert!(response
        .headers
        .iter()
        .any(|(k, v)| k == "Location" && v == "/new-location"));
}

/// WHY: Verify server stops cleanly when dropped
/// WHAT: Tests Drop implementation
#[test]
fn test_server_drop() {
    {
        let _server = TestHttpServer::start();
        // Server running
    }
    // Server should have stopped after drop
    // No assertion needed - test passes if no panic/hang
}

/// One client action in a scripted exchange with the server.
enum Step {
    /// Write these bytes to the socket.
    Write(&'static [u8]),
    /// Wait before the next step, so the server sees a partial request.
    Pause(Duration),
    /// Read and check the `100 Continue` interim response.
    ReadInterim,
}

/// Runs `steps` against a server that records every request body it
/// hands to its handler, then reads until the server closes the
/// connection. A request to `/last` is answered with `Connection: close`.
/// Returns the recorded bodies (in order) and everything the server sent.
fn exchange(steps: &[Step]) -> (Vec<Vec<u8>>, String) {
    use std::io::Read;

    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let recorder = Arc::clone(&seen);
    let server = TestHttpServer::with_response(move |req| {
        let captured = match &req.body {
            SendSafeBody::Bytes(bytes) => bytes.clone(),
            other => panic!("the server must hand handlers a byte body, got {other:?}"),
        };
        recorder.lock().unwrap().push(captured);
        let mut response = HttpResponse::ok(b"done");
        if req.path.url == "/last" {
            response
                .headers
                .push(("Connection".to_string(), "close".to_string()));
        }
        response
    });

    let addr = server.base_url().trim_start_matches("http://").to_string();
    let mut client = TcpStream::connect(&addr).expect("connect to test server");
    client
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("bound the client's reads");
    for step in steps {
        match step {
            Step::Write(bytes) => {
                client.write_all(bytes).expect("write to the server");
                client.flush().expect("flush to the server");
            }
            Step::Pause(delay) => thread::sleep(*delay),
            Step::ReadInterim => {
                let interim = b"HTTP/1.1 100 Continue\r\n\r\n";
                let mut buf = vec![0u8; interim.len()];
                client
                    .read_exact(&mut buf)
                    .expect("read the interim response");
                assert_eq!(buf, interim, "the server must answer Expect: 100-continue");
            }
        }
    }

    let mut response = String::new();
    client
        .read_to_string(&mut response)
        .expect("read the server's responses until it closes");
    let bodies = std::mem::take(&mut *seen.lock().unwrap());
    (bodies, response)
}

/// Number of complete `200 OK` responses in `raw`.
fn ok_responses(raw: &str) -> usize {
    raw.matches("HTTP/1.1 200 OK\r\n").count()
}

/// WHY: the server read bodies from a non-blocking socket with a bounded
/// number of immediate retries, so a body that arrived a moment after the
/// headers reached the handler empty (the flaky body capture in
/// foundation_ai's provider_fault_injection_tests).
/// WHAT: a `Content-Length` body sent 200ms after the headers is captured
/// in full.
#[test]
fn late_body_is_captured_in_full() {
    let (bodies, response) = exchange(&[
        Step::Write(
            b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Type: application/octet-stream\r\nContent-Length: 15\r\n\r\n",
        ),
        Step::Pause(Duration::from_millis(200)),
        Step::Write(b"late body bytes"),
    ]);
    assert_eq!(bodies, vec![b"late body bytes".to_vec()]);
    assert_eq!(ok_responses(&response), 1, "response: {response}");
}

/// WHY: a `Content-Length` body that arrives in pieces must be read to its
/// declared length, not to the end of the first read.
/// WHAT: a body written in two halves 100ms apart is captured in full.
#[test]
fn body_split_across_writes_is_captured_in_full() {
    let (bodies, response) = exchange(&[
        Step::Write(b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Length: 23\r\n\r\nfirst half, "),
        Step::Pause(Duration::from_millis(100)),
        Step::Write(b"second half"),
    ]);
    assert_eq!(bodies, vec![b"first half, second half".to_vec()]);
    assert_eq!(ok_responses(&response), 1, "response: {response}");
}

/// WHY: our own client sends `Expect: 100-continue` and writes the body
/// only after the interim response, so the body can never be there when
/// the headers are parsed.
/// WHAT: the body sent after `100 Continue` is captured in full.
#[test]
fn body_after_expect_continue_is_captured_in_full() {
    let (bodies, response) = exchange(&[
        Step::Write(
            b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: 26\r\nExpect: 100-continue\r\n\r\n",
        ),
        Step::ReadInterim,
        Step::Pause(Duration::from_millis(50)),
        Step::Write(b"{\"input\":\"after continue\"}"),
    ]);
    assert_eq!(bodies, vec![b"{\"input\":\"after continue\"}".to_vec()]);
    assert_eq!(ok_responses(&response), 1, "response: {response}");
}

/// WHY: chunked bodies have no length up front; the server must read to
/// the last chunk even when the chunks arrive over time.
/// WHAT: a chunked body sent as three writes is captured in full.
#[test]
fn chunked_body_arriving_over_time_is_captured_in_full() {
    let (bodies, response) = exchange(&[
        Step::Write(
            b"POST /last HTTP/1.1\r\nHost: test\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n",
        ),
        Step::Pause(Duration::from_millis(100)),
        Step::Write(b"6\r\n world\r\n"),
        Step::Pause(Duration::from_millis(100)),
        Step::Write(b"0\r\n\r\n"),
    ]);
    assert_eq!(bodies, vec![b"hello world".to_vec()]);
    assert_eq!(ok_responses(&response), 1, "response: {response}");
}

/// WHY: parsing a request off a non-blocking socket before its bytes
/// arrived ended the connection without an answer.
/// WHAT: a request whose head arrives late, and split in two, is answered.
#[test]
fn late_and_split_request_head_is_answered() {
    let (bodies, response) = exchange(&[
        Step::Pause(Duration::from_millis(200)),
        Step::Write(b"POST /last HTTP/1.1\r\nHost: te"),
        Step::Pause(Duration::from_millis(100)),
        Step::Write(b"st\r\nContent-Length: 4\r\n\r\nbody"),
    ]);
    assert_eq!(bodies, vec![b"body".to_vec()]);
    assert_eq!(ok_responses(&response), 1, "response: {response}");
}

/// WHY: an idle keep-alive connection was closed as soon as the next
/// request was not already buffered, so a client reusing it lost the
/// request.
/// WHAT: a second request sent 200ms after the first is answered on the
/// same connection.
#[test]
fn keep_alive_connection_serves_a_later_request() {
    let (bodies, response) = exchange(&[
        Step::Write(b"POST /first HTTP/1.1\r\nHost: test\r\nContent-Length: 3\r\n\r\none"),
        Step::Pause(Duration::from_millis(200)),
        Step::Write(b"POST /last HTTP/1.1\r\nHost: test\r\nContent-Length: 3\r\n\r\ntwo"),
    ]);
    assert_eq!(bodies, vec![b"one".to_vec(), b"two".to_vec()]);
    assert_eq!(ok_responses(&response), 2, "response: {response}");
}
