//! Feature 37 — a completion socket (and openssl/native-tls) stop answering
//! `Unsupported` from `ConnIo`.
//!
//! WHY: `Connection::conn_readiness` returns `None` for the completion socket and
//! the openssl/native-tls backends, because they cannot report readiness the way
//! a kernel fd or the overlay can. `ConnIo` used to turn that into an
//! `Unsupported` error, so an RPC over one of those transports failed as soon as
//! a read or write would block. Feature 37 makes `ConnIo` park on the valtron
//! timer and re-probe at a configured poll-fallback interval instead.
//!
//! WHAT: a deterministic mock completion socket (a Unix socketpair wrapped in a
//! `CompletionReadWrite` whose first write answers `WouldBlock`) drives a unary
//! RPC through `ConnectRpcServe::serve`. The response write must retry after one
//! fallback interval and complete with a 200, not fail with `Unsupported`. The
//! second test pins the interval as configuration.

use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::SocketAddr as UnixSocketAddr;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::serve::Serve;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::ServeOutcome;
use foundation_netio::netcap::{CompletionReadWrite, Connection, RawStream, SocketAddr};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleMethod,
};

use foundation_connectrpc::shared::codec::CodecFor;
use foundation_connectrpc::{
    ConnectRpcServe, HandlerOptions, JsonCodec, ProcedureCodecs, Request, Response, Router,
    ServeTimeouts,
};

const ECHO: &str = "http://t/test.EchoService/Echo";

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Msg {
    text: String,
}

/// A completion socket mock: a Unix socketpair end whose `read`/`write` delegate
/// to the fd, but whose first `write` answers `WouldBlock` — the one fallback
/// cycle this test pins. `is_kernel_read` is true, exactly like the `io_uring`
/// completion socket, which is why `conn_readiness` answers `None`.
#[derive(Debug)]
struct MockCompletion {
    inner: UnixStream,
    local: UnixSocketAddr,
    peer: UnixSocketAddr,
    first_write: AtomicBool,
}

impl Read for MockCompletion {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for MockCompletion {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // One `WouldBlock`, then delegate. This is what makes the fallback
        // observable: the first flush fails, `IoReady` parks for one interval,
        // and the retry succeeds.
        if self.first_write.swap(false, Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "mock completion write not ready yet",
            ));
        }
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl AsRawFd for MockCompletion {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl CompletionReadWrite for MockCompletion {
    fn peer_addr(&self) -> Option<SocketAddr> {
        Some(SocketAddr::Unix(self.peer.clone()))
    }

    fn local_addr(&self) -> io::Result<Option<SocketAddr>> {
        Ok(Some(SocketAddr::Unix(self.local.clone())))
    }

    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    fn is_kernel_read(&self) -> bool {
        true
    }
}

fn echo_serve() -> ConnectRpcServe {
    let mut router = Router::new();
    router.unary(
        ECHO,
        ProcedureCodecs::<Msg, Msg>::of((JsonCodec,)),
        |_ctx, req: Request<Msg>| async move {
            Ok(Response::new(Msg {
                text: format!("echo:{}", req.msg.text),
            }))
        },
        HandlerOptions::new(),
    );
    ConnectRpcServe::new(router.into_handler())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read an HTTP/1.1 response with a `Content-Length` body off a blocking peer
/// end: `(status, body)`.
fn read_response(stream: &mut UnixStream) -> (u16, Vec<u8>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let head_end = loop {
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = stream.read(&mut tmp).expect("read head");
        assert!(n > 0, "connection closed before headers completed");
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .expect("status line");
    let content_length = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim().eq_ignore_ascii_case("content-length"))
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
        .expect("content-length present");
    while buf.len() < head_end + content_length {
        let n = stream.read(&mut tmp).expect("read body");
        assert!(n > 0, "connection closed before body completed");
        buf.extend_from_slice(&tmp[..n]);
    }
    (status, buf[head_end..head_end + content_length].to_vec())
}

/// A transport whose `conn_readiness()` is `None` still completes an RPC: the
/// response write retries after one poll-fallback interval instead of failing
/// with `Unsupported`.
#[test]
#[foundation_macros::timeout(60000)]
fn conn_io_fallback_polls_instead_of_unsupported() {
    let (server_side, mut peer) = UnixStream::pair().expect("socketpair");
    server_side
        .set_nonblocking(true)
        .expect("non-blocking server side");
    peer.set_read_timeout(Some(Duration::from_secs(10)))
        .expect("peer read timeout");

    let local = server_side.local_addr().expect("server local addr");
    let peer_addr = server_side.peer_addr().expect("server peer addr");
    let mock = MockCompletion {
        inner: server_side,
        local,
        peer: peer_addr,
        first_write: AtomicBool::new(true),
    };

    let raw = RawStream::from_connection(Connection::Completion(Box::new(mock)))
        .expect("raw stream from completion socket");
    let conn = SharedByteBufferStream::rwrite(raw);

    let msg = Msg {
        text: "hello".to_string(),
    };
    let json = JsonCodec.marshal(&msg).expect("marshal").to_vec();
    let request = SimpleIncomingRequest::builder()
        .with_parsed_url(ECHO)
        .with_proto(Proto::HTTP11)
        .with_method(SimpleMethod::POST)
        .add_header_raw(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_body(SendSafeBody::Bytes(json))
        .build()
        .expect("request");

    let serve = echo_serve();
    let result = serve.serve(
        Arc::new(ContextBag::from_ctx(&foundation_core::valtron::Ctx::host())),
        request,
        conn,
    );
    let result = match result.split() {
        Ok(fut) => futures::executor::block_on(fut),
        Err(outcome) => outcome,
    };
    match result {
        ServeOutcome::Keep => {}
        ServeOutcome::Close(_) => {
            panic!("the RPC failed to write its response; the fallback must not answer Unsupported")
        }
        ServeOutcome::Take => panic!("unexpected Take"),
    }

    let (status, body) = read_response(&mut peer);
    assert_eq!(
        status, 200,
        "the unary over a completion socket returns 200"
    );
    let echoed: Msg = JsonCodec.unmarshal(Bytes::from(body)).expect("decode echo");
    assert_eq!(
        echoed,
        Msg {
            text: "echo:hello".to_string()
        }
    );
}

/// The poll-fallback interval is configuration, with the documented default
/// (`POLL_DELAY` of 10 ms in `native/transport/h2.rs`), and the builder reads it
/// back.
#[test]
fn conn_io_fallback_interval_is_config() {
    let default = ServeTimeouts::default();
    assert_eq!(default.poll_fallback_interval, Duration::from_millis(10));

    let custom = Duration::from_millis(250);
    let configured = default.with_poll_fallback_interval(custom);
    assert_eq!(configured.poll_fallback_interval, custom);
}
