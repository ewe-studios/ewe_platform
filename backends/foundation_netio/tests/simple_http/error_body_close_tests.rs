//! Regression tests for issue #1172: a peer that writes its whole response and
//! closes before the client sets the body-read timeout must not fail an
//! exchange whose response the client already read.
//!
//! WHY: macOS (like the other BSDs) answers `EINVAL` to `setsockopt` with
//! `SO_RCVTIMEO` / `SO_SNDTIMEO` once the peer has shut the socket down — a
//! server that sent `Connection: close` and reset the connection while the
//! response is still buffered. The timeout cannot matter then: every read
//! returns the buffered bytes at once, or reports the close normally.
//!
//! HOW: one deterministic test drives `Connection` directly: the peer writes a
//! response, closes with the client's request unread (so the close is a
//! reset), and the client calls the timeout setters after the reset arrives.
//! One end-to-end test drives `NativeHttpClient::send_async` against such a
//! server and reads the response.

use foundation_core::valtron::valtron_test;
use foundation_netio::shared::http::Status;
use std::net::SocketAddr;
use std::time::Duration;

/// A complete `400 Bad Request` response with `Content-Length` and
/// `Connection: close`, as a server that closes at once sends it.
const RESPONSE: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno";

/// A raw-TCP server that accepts, does not read the request, writes `RESPONSE`
/// and closes. Leaving the client's request unread makes the close a reset.
#[cfg(unix)]
fn serve_write_then_close() -> SocketAddr {
    use std::io::Write;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Let the client's request arrive and the client park in its head read
        // before the response and the reset. This is the `Connection: close`
        // timing that made macOS answer `EINVAL` in `hand_off`.
        std::thread::sleep(Duration::from_millis(100));
        stream.write_all(RESPONSE).expect("write response");
        stream.flush().expect("flush response");
        // Do not read the request: dropping the socket with it unread resets
        // the connection, the state that makes macOS answer `EINVAL`.
    });
    addr
}

/// A deterministic check of the decision in `Connection`: after the peer reset
/// arrives, the timeout setters must succeed and the buffered response must
/// still be readable.
///
/// Before the fix, the TCP arm called the socket directly, so `EINVAL` escaped
/// and `hand_off` failed a response the client had already read.
#[cfg(unix)]
#[test]
fn timeout_set_succeeds_after_peer_close_with_body_buffered() {
    use foundation_core::io::ioutils::ReadTimeoutOperations;
    use foundation_netio::native::connection::Connection;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Instant;

    let addr = serve_write_then_close();
    let mut raw = TcpStream::connect(addr).expect("connect");
    // Send a request the server never reads. It resets the connection on close.
    raw.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .expect("write request");
    raw.flush().expect("flush request");

    // Wait until the reset is pending. Reading `SO_ERROR` reports the peer
    // close; it also clears the pending error, which leaves the buffered
    // response readable before the end of stream.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut closed = None;
    while Instant::now() < deadline {
        match raw.take_error() {
            Ok(Some(err)) => {
                closed = Some(err);
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(err) => panic!("take_error failed: {err}"),
        }
    }
    assert!(
        closed.is_some(),
        "the peer reset never arrived; the test cannot exercise the macOS EINVAL state"
    );

    // The peer closed before the client sets the body-read timeout. On macOS a
    // direct socket call would answer EINVAL here.
    let mut conn = Connection::from(raw);
    conn.set_read_timeout_as(Duration::from_secs(5))
        .expect("read timeout on a peer-closed socket must not fail");
    conn.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set_read_timeout on a peer-closed socket must not fail");
    conn.set_write_timeout(Some(Duration::from_secs(5)))
        .expect("set_write_timeout on a peer-closed socket must not fail");

    // The response the peer already wrote is still readable with the default
    // (no) timeout.
    let mut body = vec![0_u8; RESPONSE.len()];
    conn.read_exact(&mut body)
        .expect("the buffered response must stay readable");
    assert_eq!(body, RESPONSE);
}

/// Read an async response body to the end.
#[cfg(unix)]
async fn collect_async_body(
    mut body: foundation_netio::shared::client::body_reader::AsyncSendSafeBody,
) -> Vec<u8> {
    use core::future::poll_fn;
    use core::pin::Pin;
    use futures_core::Stream;

    let mut all = Vec::new();
    loop {
        match poll_fn(|cx| Pin::new(&mut body).poll_next(cx)).await {
            Some(Ok(chunk)) => all.extend(chunk),
            Some(Err(err)) => panic!("body read failed after the response: {err}"),
            None => break,
        }
    }
    all
}

/// End-to-end: a server writes its whole response and closes before the client
/// sets the body-read timeout. `send_async` must return the response, not
/// `HttpClientError::IoError(Invalid argument)`.
#[cfg(unix)]
#[valtron_test]
async fn send_async_returns_the_response_from_a_peer_that_closed() {
    use foundation_core::url::Uri;
    use foundation_netio::http::NativeHttpClient;
    use foundation_netio::shared::client::http_client::HttpClient;
    use foundation_netio::shared::client::{PreparedRequest, SystemDnsResolver};
    use foundation_netio::shared::http::{
        Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    };

    let addr = serve_write_then_close();
    let url = format!("http://127.0.0.1:{}/closed", addr.port());
    let uri = Uri::parse(&url).expect("uri");
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::HOST,
        vec![format!("127.0.0.1:{}", addr.port())],
    );
    let request = PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    };

    let client = NativeHttpClient::<SystemDnsResolver>::from_system();
    let response = client
        .send_async(request)
        .await
        .expect("a peer close before the timeout set must not fail the response");

    let (status, _headers, body) = response.into_async_parts();
    assert_eq!(status, Status::BadRequest);
    assert_eq!(collect_async_body(body).await, b"no");
}
