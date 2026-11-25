//! A mock HTTP server on a Unix socket, for the Firecracker API tests.
//!
//! WHY: The Firecracker API is Hypertext Transfer Protocol (HTTP) over a Unix
//! domain socket (UDS). A unit test needs a server it controls, so it can
//! answer a status, a body or nothing at all.
//!
//! WHAT: [`serve`] runs an accept loop on a blocking listener until [`stop`]
//! sets the flag. [`read_request`] reads a whole request: the head through its
//! blank line, then a `Content-Length` body, answering `Expect: 100-continue`
//! first.
//!
//! HOW: This is the mock of `foundation_deployment_platform` and
//! `foundation_deployment_docker`, copied so this crate has no test dependency
//! on a deployment crate.

#![allow(
    dead_code,
    reason = "each test module uses a different subset of these helpers"
)]

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The largest request head the mock reads; the client's requests are a few
/// hundred bytes.
const MAX_HEAD: usize = 64 * 1024;

/// One request, as the mock server received it.
#[derive(Debug)]
pub struct Request {
    /// `GET`, `PUT`, …
    pub method: String,
    /// The request target: path and query.
    pub target: String,
    /// The `Content-Length` body (empty when there is none).
    pub body: Vec<u8>,
}

/// Bind `socket` and hand every accepted connection to `handle`, on one
/// accept thread, until [`stop`] is called.
///
/// `handle` runs on the accept thread. A handler that holds a connection open
/// holds the later ones too; one connection per test is enough.
///
/// # Panics
///
/// If the socket cannot be bound.
pub fn serve<H>(socket: &Path, stop: Arc<AtomicBool>, handle: H)
where
    H: Fn(UnixStream) + Send + 'static,
{
    let listener = UnixListener::bind(socket).expect("bind the mock API socket");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            match conn {
                Ok(stream) => handle(stream),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
}

/// Stop the server [`serve`] started on `socket`: set `flag`, wake the accept
/// loop (blocked in `accept`) with a connection of its own, and remove the
/// socket file.
pub fn stop(socket: &Path, flag: &AtomicBool) {
    flag.store(true, Ordering::Relaxed);
    // Fails only if the loop has already gone; nothing to wake then.
    let _ = UnixStream::connect(socket);
    let _ = std::fs::remove_file(socket);
}

/// Read one request from `stream`: its head, then its `Content-Length` body.
/// An `Expect: 100-continue` request is told to continue before its body is
/// read. `None` if the peer closed or sent something that is not a request.
pub fn read_request(stream: &mut UnixStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > MAX_HEAD {
            return None;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let target = request_line.next()?.to_string();

    let mut content_length = 0usize;
    let mut expect_continue = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse().ok()?;
        } else if name.eq_ignore_ascii_case("expect") && value.eq_ignore_ascii_case("100-continue")
        {
            expect_continue = true;
        }
    }

    let mut body = buf.split_off(head_end);
    if expect_continue && content_length > body.len() {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").ok()?;
        stream.flush().ok()?;
    }
    while body.len() < content_length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Some(Request {
        method,
        target,
        body,
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
