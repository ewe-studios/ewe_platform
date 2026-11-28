//! A kqueue read edge must never be suppressed on a socket.
//!
//! WHY: GitHub issue #1218. The kqueue selector records the byte count it last
//! delivered and suppresses a plain-readable event whose count did not grow.
//! That rule exists because macOS reports `EVFILT_READ` level-triggered for
//! pipes despite `EV_CLEAR`. It is wrong for sockets, where the kernel is
//! edge-triggered: a 50-byte frame after a drained 100-byte frame is a real
//! edge, but its count is smaller, so the selector dequeued and dropped it.
//! `EV_CLEAR` had already consumed the kernel edge, so no later re-arm could
//! deliver it — the task parked until its deadline.
//!
//! WHAT: two tests. The first reproduces the loss on a loopback TCP pair: write
//! a large frame, drain it, write a smaller frame, and require the selector to
//! report the smaller frame. It fails before the fix. The second pins the pipe
//! behaviour the suppression exists for: macOS re-reports a partly read pipe,
//! the selector suppresses that duplicate, and a re-arm plus a new write
//! delivers a fresh edge.
//!
//! HOW: drive the kqueue `Selector` directly, so the test owns the poll and no
//! drain thread can race it. Each write completes before the poll starts, so the
//! kernel edge is already queued and the outcome is deterministic.

#![cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use std::time::Duration;

use foundation_reactor::poll::sys::unix::selector::kqueue::Selector;
use foundation_reactor::{Events, Interest, Token};

/// A generous wait: the data is queued before the poll starts, so a correct
/// selector returns at once and a broken one waits out this bound.
const WAIT: Duration = Duration::from_millis(250);

/// A connected loopback TCP pair. The reader stays blocking: the test reads only
/// after the selector reports it readable.
fn loopback_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback");
    let address = listener.local_addr().expect("loopback address");
    let reader = TcpStream::connect(address).expect("connect loopback");
    let (peer, _) = listener.accept().expect("accept loopback");
    (reader, peer)
}

/// A pipe owned as two descriptors.
fn pipe_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` points at two `c_int`s, which is what `pipe(2)` writes.
    let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(result, 0, "pipe(2) failed: {}", io::Error::last_os_error());
    // SAFETY: `pipe` returned two descriptors that this process now owns, and
    // each `OwnedFd` takes exactly one.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

/// Poll once and return the delivered events.
fn poll_readable(selector: &Selector, label: &str) -> Events {
    let mut events = Events::with_capacity(8);
    selector
        .poll(&mut events, Some(WAIT))
        .unwrap_or_else(|error| panic!("{label}: poll failed: {error}"));
    events
}

/// Write all of `bytes` to a raw fd.
fn write_fd(fd: std::os::fd::RawFd, bytes: &[u8]) {
    let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
    assert_eq!(
        written,
        libc::ssize_t::try_from(bytes.len()).expect("buffer length fits ssize_t"),
        "write to the pipe failed: {}",
        io::Error::last_os_error()
    );
}

/// Read up to `buffer.len()` bytes from a raw fd.
fn read_fd(fd: std::os::fd::RawFd, buffer: &mut [u8]) -> usize {
    let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
    assert!(
        read >= 0,
        "read from the pipe failed: {}",
        io::Error::last_os_error()
    );
    usize::try_from(read).expect("a non-negative read count fits usize")
}

/// A smaller socket frame after a drained larger frame must be delivered.
///
/// Before the fix the selector holds the larger frame's byte count, treats the
/// smaller frame as "no new data", and drops it. The kernel edge is already
/// consumed, so the wake is lost for good.
#[test]
fn a_smaller_socket_frame_after_a_drain_is_delivered() {
    let (mut reader, mut peer) = loopback_pair();
    let selector = Selector::new().expect("create the selector");
    selector
        .register_fd(reader.as_raw_fd(), Token(1), Interest::READABLE)
        .expect("register the reader");

    // First edge: a large frame, delivered and drained.
    peer.write_all(&[b'A'; 1024]).expect("write the large frame");
    let head = poll_readable(&selector, "large frame");
    assert_eq!(head.len(), 1, "the large frame must be delivered");
    let mut buffer = [0_u8; 1024];
    reader.read_exact(&mut buffer).expect("drain the large frame");

    // Second edge: a smaller frame. Its byte count is smaller, but it is a new
    // kernel edge and must be delivered.
    peer.write_all(&[b'B'; 16]).expect("write the small frame");
    let frame = poll_readable(&selector, "small frame");
    assert_eq!(
        frame.len(),
        1,
        "the selector suppressed a 16-byte socket edge after a drained \
         1024-byte edge; a caller that re-arms later cannot recover it"
    );
    assert!(
        frame.iter().any(|event| event.is_readable()),
        "the delivered socket event must be readable"
    );
}

/// The pipe edge workaround keeps working: macOS re-reports a pipe after a
/// partial read, and the selector must suppress that duplicate until a re-arm.
#[test]
fn a_pipe_read_edge_is_still_suppressed_after_a_partial_read() {
    let (reader, writer) = pipe_pair();
    let selector = Selector::new().expect("create the selector");
    selector
        .register_fd(reader.as_raw_fd(), Token(1), Interest::READABLE)
        .expect("register the pipe");

    write_fd(writer.as_raw_fd(), &[b'A'; 5]);
    let first = poll_readable(&selector, "first pipe edge");
    assert_eq!(first.len(), 1, "the first pipe edge must be delivered");

    // Leave 3 of the 5 bytes unread. macOS queues another event at the new
    // level; the selector must suppress that duplicate.
    let mut two = [0_u8; 2];
    assert_eq!(read_fd(reader.as_raw_fd(), &mut two), 2);
    let level = poll_readable(&selector, "pipe level re-report");
    assert!(
        level.is_empty(),
        "a pipe re-report without new data must stay suppressed"
    );

    // Drain the rest, then re-arm as the reactor's clear does. A new write is
    // a fresh edge and must be delivered.
    let mut rest = [0_u8; 8];
    assert_eq!(read_fd(reader.as_raw_fd(), &mut rest), 3);
    selector.rearm_readable(reader.as_raw_fd());
    write_fd(writer.as_raw_fd(), &[b'B'; 1]);
    let next = poll_readable(&selector, "new pipe edge");
    assert_eq!(
        next.len(),
        1,
        "a new pipe write after a re-arm must be delivered"
    );
}
