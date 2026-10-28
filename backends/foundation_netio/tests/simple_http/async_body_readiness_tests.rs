//! Feature 03: async body reads park on socket readiness instead of spinning.
//!
//! WHY: `AsyncSendSafeBody::poll_next` used to call `cx.waker().wake_by_ref()`
//! and return `Pending` on every "nothing yet", so an async caller busy-looped
//! while the server held the body back. Now the body carries a
//! `ReadWakeSource` (Feature 29): the waker is registered with the shared
//! reactor (or the overlay driver) and the reactor wakes it when the socket
//! becomes readable. The poll count during a wait must stay tiny.
//!
//! HOW: a raw-TCP server sends the response head, sleeps, then sends the body.
//! The client drives `NativeHttpClient::send_async`, converts the body with
//! `SimpleResponse::into_async_parts`, and polls it through a counting wrapper.
//! The count is the number of `poll_next` calls during the wait.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::url::Uri;
use foundation_core::valtron::{self, valtron_test};
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::{PreparedRequest, SystemDnsResolver};
use foundation_netio::shared::http::{
    Extensions, HttpReaderError, HttpResponseReader, IncomingResponseParts, SendSafeBody,
    SimpleHeader, SimpleHeaders, SimpleHttpBody, SimpleMethod, Status,
};

use core::future::poll_fn;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::Stream as FuturesStream;

/// CPU time used so far by the calling thread, in nanoseconds.
///
/// The body read parks on the valtron worker thread, and `#[valtron_test]`
/// gives each test a fresh pool, so the polling thread's CPU isolates the wait
/// from the CPU of any other test running in parallel on the harness threads.
#[cfg(unix)]
fn thread_cpu_nanos() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid out pointer; CLOCK_THREAD_CPUTIME_ID is supported
    // on every unix target this crate tests on.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

fn get_request(url: &str) -> PreparedRequest {
    let uri = Uri::parse(url).expect("uri");
    let host = match uri.port() {
        Some(p) => format!(
            "{}:{p}",
            uri.host_str().unwrap_or_else(|| "127.0.0.1".to_string())
        ),
        None => uri.host_str().unwrap_or_else(|| "127.0.0.1".to_string()),
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

/// A raw-TCP server that writes a whole response head, sleeps `delay`, writes
/// `body`, and closes. Returns the listening address.
fn serve(head: String, body: Vec<u8>, delay: Duration) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        if let Some(stream) = listener.incoming().next() {
            let mut stream = stream.expect("accept");
            let mut buf = [0u8; 4096];
            let _n = stream.read(&mut buf).unwrap_or(0);
            stream.write_all(head.as_bytes()).expect("write head");
            stream.flush().expect("flush head");
            std::thread::sleep(delay);
            stream.write_all(&body).expect("write body");
            stream.flush().expect("flush body");
        }
    });
    addr
}

/// A raw-TCP server that accepts, reads the request, and then holds the
/// connection open without answering. Used to make a blocking read time out.
fn serve_silent() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _n = stream.read(&mut buf).unwrap_or(0);
            // Never respond and never close: the client's read must time out.
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    addr
}

/// A `Stream` wrapper that counts `poll_next` calls and records the polling
/// thread's CPU at the first and last poll, and delegates to the body.
struct CountingBody {
    inner: AsyncSendSafeBody,
    polls: Arc<AtomicUsize>,
    cpu_first: Arc<AtomicU64>,
    cpu_last: Arc<AtomicU64>,
}

impl FuturesStream for CountingBody {
    type Item = Result<Vec<u8>, SendableBoxedError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Record the polling thread's CPU before each poll: the first poll sets
        // the baseline, the last poll measures the wait on that worker thread.
        let now = thread_cpu_nanos();
        if self.polls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.cpu_first.store(now, Ordering::SeqCst);
        }
        self.cpu_last.store(now, Ordering::SeqCst);
        // SAFETY: we do not move out of `self`; `inner` stays in place.
        let this = unsafe { self.get_unchecked_mut() };
        Pin::new(&mut this.inner).poll_next(cx)
    }
}

/// Read a `CountingBody` to the end, returning the bytes, the poll count, and
/// the polling thread's CPU at the first and last poll.
async fn collect_counted(
    body: CountingBody,
) -> (Vec<u8>, Arc<AtomicUsize>, Arc<AtomicU64>, Arc<AtomicU64>) {
    let polls = body.polls.clone();
    let cpu_first = body.cpu_first.clone();
    let cpu_last = body.cpu_last.clone();
    let mut counted = body;
    let mut all = Vec::new();
    loop {
        let chunk = poll_fn(|cx| Pin::new(&mut counted).poll_next(cx)).await;
        match chunk {
            Some(Ok(bytes)) => all.extend(bytes),
            Some(Err(e)) => panic!("body read failed: {e}"),
            None => break,
        }
    }
    (all, polls, cpu_first, cpu_last)
}

/// Drive a client response through `into_async_parts`, count the `poll_next`
/// calls, and return the bytes, poll count, elapsed time and polling-thread CPU.
#[cfg(unix)]
async fn read_with_counting(
    response: foundation_netio::shared::http::SimpleResponse<SendSafeBody>,
) -> (Vec<u8>, usize, Duration, u128) {
    let (status, _headers, body) = response.into_async_parts();
    assert_eq!(status, Status::OK);
    let polls = Arc::new(AtomicUsize::new(0));
    let cpu_first = Arc::new(AtomicU64::new(0));
    let cpu_last = Arc::new(AtomicU64::new(0));
    let counted = CountingBody {
        inner: body,
        polls: polls.clone(),
        cpu_first: cpu_first.clone(),
        cpu_last: cpu_last.clone(),
    };
    let started = Instant::now();
    let (bytes, _, _, _) = collect_counted(counted).await;
    let elapsed = started.elapsed();
    let count = polls.load(Ordering::SeqCst);
    let cpu_ns = cpu_last
        .load(Ordering::SeqCst)
        .saturating_sub(cpu_first.load(Ordering::SeqCst));
    (bytes, count, elapsed, cpu_ns as u128)
}

#[cfg(unix)]
fn assert_parked(bytes: Vec<u8>, expected: &[u8], count: usize, elapsed: Duration, cpu_ns: u128) {
    assert_eq!(bytes, expected);
    assert!(
        count < 10,
        "poll_next ran {count} times during the wait; it should park, not spin"
    );
    assert!(
        elapsed >= Duration::from_millis(400),
        "the body arrived after {elapsed:?}; it should have waited for the server's delay"
    );
    let cpu_ms = cpu_ns / 1_000_000;
    assert!(
        cpu_ms < 50,
        "reading the body used {cpu_ms} ms of CPU; it should park on readiness"
    );
}

#[cfg(unix)]
#[valtron_test]
async fn async_body_waits_without_spinning() {
    let addr = serve(
        format!("HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n"),
        b"hello body".to_vec(),
        Duration::from_millis(500),
    );
    let url = format!("http://127.0.0.1:{}/test", addr.port());
    let client = NativeHttpClient::<SystemDnsResolver>::from_system();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (bytes, count, elapsed, cpu_ns) = read_with_counting(response).await;
    assert_parked(bytes, b"hello body", count, elapsed, cpu_ns);
}

#[cfg(unix)]
#[valtron_test]
async fn async_body_chunked_waits_without_spinning() {
    // Chunked transfer encoding.
    let chunked = {
        let mut out = Vec::new();
        for piece in [b"hello ".as_slice(), b"chunked".as_slice()] {
            out.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
            out.extend_from_slice(piece);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"0\r\n\r\n");
        out
    };
    let addr = serve(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_string(),
        chunked,
        Duration::from_millis(500),
    );
    let url = format!("http://127.0.0.1:{}/chunked", addr.port());
    let client = NativeHttpClient::<SystemDnsResolver>::from_system();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (bytes, count, elapsed, cpu_ns) = read_with_counting(response).await;
    assert_parked(bytes, b"hello chunked", count, elapsed, cpu_ns);
}

#[cfg(unix)]
#[valtron_test]
async fn async_body_sse_waits_without_spinning() {
    // An SSE response (`text/event-stream`) streams one `data:` event after the
    // delay; the body reader parses the event and yields its data.
    let addr = serve(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
            .to_string(),
        b"data: hello\n\n".to_vec(),
        Duration::from_millis(500),
    );
    let url = format!("http://127.0.0.1:{}/sse", addr.port());
    let client = NativeHttpClient::<SystemDnsResolver>::from_system();

    let response = client.send_async(get_request(&url)).await.expect("send");
    let (bytes, count, elapsed, cpu_ns) = read_with_counting(response).await;
    // `AsyncSendSafeBody` yields each SSE message's `data` plus a newline.
    assert_parked(bytes, b"hello\n", count, elapsed, cpu_ns);
}

#[cfg(unix)]
#[test]
fn blocking_socket_wouldblock_is_an_error() {
    // A blocking socket with a short read timeout and a server that never sends:
    // the read timeout must surface as an error, not a hang and not a retry.
    let addr = serve_silent();
    let tcp = TcpStream::connect(addr).expect("connect");
    tcp
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("read timeout");
    tcp.set_nonblocking(false).expect("blocking mode");

    let stream = SharedByteBufferStream::ref_cell(tcp);
    let mut reader = HttpResponseReader::new(stream, SimpleHttpBody::default());

    let started = Instant::now();
    let first = reader.next();
    let elapsed = started.elapsed();

    match first {
        Some(Err(HttpReaderError::WouldBlock)) => {}
        other => panic!("expected a WouldBlock timeout error, got {other:?}"),
    }
    assert!(
        elapsed >= Duration::from_millis(100),
        "the read returned after {elapsed:?}; it should have waited out the timeout"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "the read hung for {elapsed:?}; a blocking timeout must be an error"
    );
}
