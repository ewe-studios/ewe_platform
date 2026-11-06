//! Feature 137: a stream builds its read wake source once.
//!
//! WHY: `RawStream::read_wake_source` used to build a new `FdReadiness` on every
//! call. Each one takes a fresh reactor token for the same fd, and kqueue and
//! epoll register per fd, not per token, so a second registration replaces the
//! first one's event registration. A waker registered through the first source
//! then never fires. The API shape invited the mistake: `read_wake_source(&self)`
//! looked idempotent and was not.
//!
//! WHAT: two loopback tests. The first asserts that two calls return the same
//! `Arc`. The second registers a counting waker through source A, creates
//! source B, sends bytes, and asserts that A's waker fires within 2 s. A third
//! test checks that a negative answer is not cached (feature 141).
//!
//! On the old code the first test compares two distinct `Arc`s and the second
//! times out because B's registration owns the fd's selector event.

#![cfg(unix)]

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};
use std::time::{Duration, Instant};

use foundation_netio::netcap::RawStream;

/// A connected loopback pair, the client side wrapped as a non-blocking
/// `RawStream`. The server stream stays open so the socket remains readable.
#[cfg(unix)]
fn loopback() -> (RawStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
    let addr = listener.local_addr().expect("loopback address");
    let client = TcpStream::connect(addr).expect("connect loopback client");
    let (server, _) = listener.accept().expect("accept loopback client");

    let stream = RawStream::from_tcp(client).expect("build raw stream");
    stream
        .set_nonblocking(true)
        .expect("put the stream in non-blocking mode");
    (stream, server)
}

/// A `Waker` that counts its wakes.
struct CountingWake(Arc<AtomicUsize>);

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(unix)]
#[test]
fn read_wake_source_is_stable_across_calls() {
    let (stream, _server) = loopback();

    let first = stream
        .read_wake_source()
        .expect("a non-blocking socket has a read wake source");
    let second = stream
        .read_wake_source()
        .expect("a second call must return the same source");

    assert!(
        Arc::ptr_eq(&first, &second),
        "read_wake_source built a second source: two sources for one fd take \
         two reactor tokens, and the second registration replaces the first"
    );
}

#[cfg(unix)]
#[test]
fn a_waker_registered_through_the_first_source_still_fires() {
    let (stream, mut server) = loopback();

    let source_a = stream
        .read_wake_source()
        .expect("a non-blocking socket has a read wake source");
    let wake_count = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(CountingWake(Arc::clone(&wake_count))));
    assert!(
        source_a(&waker),
        "the source must register the waker on a live fd registration"
    );

    // On the old code this call registers the same fd under a new token and
    // replaces source A's selector registration.
    let _source_b = stream
        .read_wake_source()
        .expect("a second call must return the same source");

    server.write_all(b"x").expect("write to the loopback pair");
    server.flush().expect("flush the loopback pair");

    let deadline = Instant::now() + Duration::from_secs(2);
    while wake_count.load(Ordering::SeqCst) == 0 {
        assert!(
            Instant::now() < deadline,
            "the waker registered through the first source did not fire within \
             2 s after a second source was created and bytes arrived"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A negative answer must not be cached (feature 141).
///
/// A stream that is still blocking answers `None`. The caller may set it
/// non-blocking later, and the next call must then build the source.
#[cfg(unix)]
#[test]
fn a_negative_answer_is_not_cached() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
    let addr = listener.local_addr().expect("loopback address");
    let client = TcpStream::connect(addr).expect("connect loopback client");
    let (_server, _) = listener.accept().expect("accept loopback client");
    let stream = RawStream::from_tcp(client).expect("build raw stream");

    assert!(
        stream.read_wake_source().is_none(),
        "a blocking socket has no readiness wait, so the answer is negative"
    );

    stream
        .set_nonblocking(true)
        .expect("put the stream in non-blocking mode");
    assert!(
        stream.read_wake_source().is_some(),
        "the negative answer was cached: a stream that became non-blocking \
         must build its source on the next call"
    );
}
