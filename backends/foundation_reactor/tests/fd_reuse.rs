//! A late deregistration must not remove a new socket's readiness.
//!
//! WHY: the selector keys a registration by fd number alone. If a socket
//! closes before its `FdReadiness` drops, the operating system can give the
//! number to a new socket. The stale drop then removes the *new* socket's
//! selector entry, and that socket never reports readiness again.
//!
//! WHAT: reuse an fd number, drop the old readiness, and assert that the new
//! registration still reports readiness. A second test keeps the normal case:
//! a live drop still removes the registration.
//!
//! HOW: each test drives real local sockets through the process-wide reactor.
//! This is its own test binary because the reactor is a process singleton. A
//! mutex makes the two tests run one at a time: both need to own a specific fd
//! number, and parallel tests would compete for it.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use foundation_reactor::{next_token, FdReadiness, Interest, Reactor, Ready, Token};

/// A wait without a correctness sleep: poll the reactor, and park on its event
/// generation between checks. The bound is generous enough for a loaded CI
/// runner and short enough to fail a lost wake quickly.
const WAIT_BOUND: Duration = Duration::from_secs(2);

/// Serialises the tests in this binary. Both need a known fd number, and the
/// second test's open sockets would stop the first from reusing its number.
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Wait until `token` reports readable on `reactor`, or fail after
/// [`WAIT_BOUND`] with the fd and token in the message.
fn wait_for_token(reactor: &Reactor, token: Token, fd: RawFd, label: &str) {
    let deadline = Instant::now() + WAIT_BOUND;
    while Instant::now() < deadline {
        // Snapshot the generation before the check: an event that lands in the
        // race window bumps it, so the wait below returns at once.
        let generation = reactor.events_generation();
        if reactor.is_ready(token) {
            return;
        }
        reactor.wait_for_events(generation, Duration::from_millis(100));
    }
    panic!("{label}: fd {fd} with {token:?} did not report readiness within {WAIT_BOUND:?}");
}

/// Wait until `ready` reports readable, or fail after [`WAIT_BOUND`]. The
/// message prints `Ready`'s debug form, which names its fd and token.
fn wait_for_readiness(ready: &FdReadiness, fd: RawFd, label: &str) {
    let deadline = Instant::now() + WAIT_BOUND;
    while Instant::now() < deadline {
        let generation = ready.events_generation();
        if ready.ready().is_readable() {
            return;
        }
        // The return is the new generation; the loop re-snapshots it, so
        // discarding it here is deliberate.
        let _ = ready.wait_for_events(generation, Duration::from_millis(100));
    }
    panic!("{label}: fd {fd} ({ready:?}) did not report readability within {WAIT_BOUND:?}");
}

/// Open socket pairs until one end owns `number`, keeping the earlier pairs
/// open so each attempt gets a higher free number. Returns the socket with
/// `number` and its peer.
fn reopen_as(number: RawFd) -> (UnixStream, UnixStream, Vec<(UnixStream, UnixStream)>) {
    let mut held = Vec::new();
    for attempt in 0..32 {
        let (left, right) = UnixStream::pair().expect("socketpair");
        let left_number = left.as_raw_fd();
        let right_number = right.as_raw_fd();
        if left_number == number {
            return (left, right, held);
        }
        if right_number == number {
            return (right, left, held);
        }
        held.push((left, right));
        assert!(
            attempt < 31,
            "could not get fd {number} back after 32 socket pairs; the test cannot reuse \
             the number and cannot prove the stale-drop case"
        );
    }
    unreachable!("the loop returns or asserts")
}

/// A drop after fd reuse must leave the new registration armed.
///
/// This test fails on the old code: the stale deregister removes the selector
/// entry for the reused number, so the new socket never reports readiness.
#[test]
fn a_drop_after_fd_reuse_leaves_the_new_registration_armed() {
    let _serial = serial();

    let (first, first_peer) = UnixStream::pair().expect("socketpair");
    first.set_nonblocking(true).expect("nonblocking first end");
    let number = first.as_raw_fd();

    let stale = FdReadiness::register(number).expect("register the first socket");
    // Closing the socket does not drop its `FdReadiness`: a waker elsewhere can
    // hold it, which is the real ordering this test reproduces.
    drop(first);

    let (mut second, mut peer, _held) = reopen_as(number);
    second
        .set_nonblocking(true)
        .expect("nonblocking second end");
    assert_eq!(
        second.as_raw_fd(),
        number,
        "the new socket must own the old fd number"
    );
    let live = FdReadiness::register(number).expect("register the second socket");

    // The late drop: the fd number now belongs to `live`, so this must not
    // touch the selector.
    drop(stale);

    peer.write_all(b"ready")
        .expect("write to the second socket");
    peer.flush().expect("flush the second socket");

    wait_for_readiness(&live, number, "a drop after fd reuse");

    let mut buf = [0u8; 5];
    second
        .read_exact(&mut buf)
        .expect("the second socket stays readable");
    assert_eq!(&buf, b"ready");

    // Keep the first peer alive until the assertions finish so its number does
    // not get reused in the middle of the test.
    drop(first_peer);
}

/// A live drop still deregisters: the number is free for a new registration,
/// and the old token never sees a later event.
#[test]
fn a_drop_of_the_live_registration_still_deregisters() {
    let _serial = serial();

    let reactor = Reactor::get().expect("reactor");
    let (socket, mut peer) = UnixStream::pair().expect("socketpair");
    socket.set_nonblocking(true).expect("nonblocking socket");
    let fd = socket.as_raw_fd();
    let token = next_token();

    reactor
        .register(fd, token, Interest::READABLE)
        .expect("register the live socket");
    peer.write_all(b"first").expect("write the first byte");
    wait_for_token(&reactor, token, fd, "live registration");
    reactor.clear(token, Ready::READABLE);

    reactor
        .deregister(fd, token)
        .expect("a live deregister must succeed");
    assert!(
        !reactor.is_ready(token),
        "fd {fd} with {token:?} still reports readiness after its deregister"
    );

    // A later event on the still-open socket must not reach the old token.
    let since = reactor.events_generation();
    peer.write_all(b"second").expect("write the second byte");
    let _ = reactor.wait_for_events(since, Duration::from_millis(300));
    assert!(
        !reactor.is_ready(token),
        "fd {fd} with {token:?} received an event after its deregister"
    );

    // The selector no longer holds the number: a new registration succeeds and
    // reports readiness. On epoll a leftover entry would fail this with
    // `EEXIST`.
    let token_after = next_token();
    reactor
        .register(fd, token_after, Interest::READABLE)
        .expect("the fd must be free after a live deregister");
    peer.write_all(b"third").expect("write the third byte");
    wait_for_token(&reactor, token_after, fd, "re-registration");
    reactor
        .deregister(fd, token_after)
        .expect("deregister the re-registration");
}
