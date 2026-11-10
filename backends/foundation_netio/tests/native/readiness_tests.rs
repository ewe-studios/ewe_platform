//! Tests for the native readiness signals (moved from `src/native/readiness.rs`).

#![cfg(unix)]

use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{Ctx, EventReadiness, EventReadinessPtr, MonotonicClock};
use foundation_netio::native::readiness::{
    io_or_deadline, DeadlineReadiness, IoInterest, SocketReadiness,
};

/// A real clock: these signals only test monotonic ordering.
fn clock() -> Ctx {
    Ctx::fixed(Arc::new(MonotonicClock::new()))
}

#[test]
fn readable_only_after_peer_writes() {
    let (a, mut b) = UnixStream::pair().expect("socket pair");
    let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Readable);
    assert!(!signal.is_ready(None), "nothing written yet");

    b.write_all(b"x").expect("peer write");
    assert!(signal.is_ready(None), "peer wrote a byte");
}

#[test]
fn hang_up_counts_as_readable() {
    let (a, b) = UnixStream::pair().expect("socket pair");
    let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Readable);
    assert!(!signal.is_ready(None));

    drop(b);
    assert!(signal.is_ready(None), "EOF must wake a parked reader");
}

#[test]
fn fresh_socket_is_writable() {
    let (a, _b) = UnixStream::pair().expect("socket pair");
    let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Writable);
    assert!(signal.is_ready(None));
}

#[test]
fn full_send_buffer_is_not_writable() {
    let (a, _b) = UnixStream::pair().expect("socket pair");
    a.set_nonblocking(true).expect("nonblocking");
    let mut writer = &a;
    let chunk = vec![0u8; 64 * 1024];
    loop {
        match writer.write(&chunk) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("unexpected write error: {e}"),
        }
    }
    let signal = SocketReadiness::new(a.as_raw_fd(), IoInterest::Writable);
    assert!(
        !signal.is_ready(None),
        "the peer never read, so the buffer is full"
    );
}

#[test]
fn deadline_fires_and_composes() {
    let (a, _b) = UnixStream::pair().expect("socket pair");
    let io: EventReadinessPtr = Arc::new(SocketReadiness::new(a.as_raw_fd(), IoInterest::Readable));

    let later = io_or_deadline(
        io.clone(),
        Instant::now() + Duration::from_secs(60),
        clock(),
    );
    assert!(
        !later.is_ready(None),
        "neither the socket nor the deadline is ready"
    );

    let past = io_or_deadline(io, Instant::now(), clock());
    assert!(past.is_ready(None), "a passed deadline wakes the task");
    assert!(!DeadlineReadiness::after(Duration::from_secs(60), clock()).expired());
}
