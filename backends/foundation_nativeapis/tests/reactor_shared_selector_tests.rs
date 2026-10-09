//! Shared-reactor wake-path tests (Decision 14 §Scope 3 / F40).
//!
//! WHY: `Reactor` is the process-level singleton that every `FdRegistration`
//! parks on. If the fds it registers are not the fds its drain thread polls,
//! `is_ready()` can never turn true and every task parked via
//! `TaskStatus::Depends(RegisteredFd)` parks forever. That failure is silent —
//! nothing errors, tasks just never wake — so it needs a direct test.
//!
//! WHAT: register one end of a pipe with the shared reactor, write to the other
//! end, and assert the reactor observes readiness within a bounded wait. Also
//! covers the edge-triggered clear/re-arm protocol and readiness bit fidelity.
//!
//! HOW: raw `pipe(2)` so the test depends on nothing but the reactor itself.
//! The reactor is a process-wide singleton, so these tests share one instance:
//! each uses a distinct `Token`, and all run `#[serial]` because the fd-count
//! test measures a process-global quantity that concurrent pipe creation in a
//! sibling test would perturb.

#![cfg(all(unix, feature = "fd"))]

use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use foundation_nativeapis::native::fd::{Ready, Reactor};
use foundation_nativeapis::{Interest, Token};

/// Create a nonblocking pipe, returning `(read_fd, write_fd)`.
fn nonblocking_pipe() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe2 to fill.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((fds[0], fds[1]))
}

/// Poll `Reactor::is_ready` until it turns true or `budget` elapses.
fn wait_ready(reactor: &Reactor, token: Token, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if reactor.is_ready(token) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// Poll until `token`'s latched readiness contains `flag`, or `budget` elapses.
fn wait_flag(reactor: &Reactor, token: Token, flag: Ready, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if reactor.readiness(token).contains(flag) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// Write one byte into `fd`, asserting the write lands.
fn write_byte(fd: RawFd) {
    // SAFETY: writing one byte from a valid local buffer to an owned pipe fd.
    let wrote = unsafe { libc::write(fd, b"x".as_ptr().cast(), 1) };
    assert_eq!(wrote, 1, "write to pipe: {}", io::Error::last_os_error());
}

/// Drain `fd` until it reports `WouldBlock`, the precondition for clearing.
fn drain_to_would_block(fd: RawFd) {
    let mut buf = [0u8; 64];
    loop {
        // SAFETY: reading into a valid local buffer from an owned pipe fd.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            let err = io::Error::last_os_error();
            assert_eq!(
                err.kind(),
                io::ErrorKind::WouldBlock,
                "unexpected read error while draining: {err}"
            );
            return;
        }
        if n == 0 {
            return;
        }
    }
}

#[test]
#[serial_test::serial]
fn shared_reactor_observes_readiness_on_registered_fd() {
    let reactor = Reactor::get().expect("shared reactor must initialise");
    let (read_fd, write_fd) = nonblocking_pipe().expect("pipe");
    let token = Token(0xF43_0001);

    reactor
        .register(read_fd, token, Interest::READABLE)
        .expect("register read end");

    assert!(
        !reactor.is_ready(token),
        "an idle pipe must not report readable before anything is written"
    );

    write_byte(write_fd);

    assert!(
        wait_ready(&reactor, token, Duration::from_secs(2)),
        "reactor never saw the pipe become readable — the drain thread is not \
         polling the selector that registrations land in"
    );

    reactor.deregister(read_fd, token).expect("deregister");
    // SAFETY: both fds are owned by this test and not used again.
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}

#[test]
#[serial_test::serial]
fn latched_readiness_clears_then_rearms_on_the_next_edge() {
    let reactor = Reactor::get().expect("shared reactor must initialise");
    let (read_fd, write_fd) = nonblocking_pipe().expect("pipe");
    let token = Token(0xF43_0002);

    reactor
        .register(read_fd, token, Interest::READABLE)
        .expect("register read end");

    // First edge.
    write_byte(write_fd);
    assert!(
        wait_ready(&reactor, token, Duration::from_secs(2)),
        "first write must latch readiness"
    );

    // Consume the data, then clear the latch — the contract a task honours when
    // its read hits WouldBlock.
    drain_to_would_block(read_fd);
    reactor.clear(token, Ready::READABLE);

    assert!(
        !reactor.is_ready(token),
        "clearing a drained fd must un-latch readiness; a sticky bit makes every \
         parked task spin instead of sleeping"
    );

    // The selector is edge-triggered: a *new* write is a new edge and must
    // re-latch without any re-registration.
    write_byte(write_fd);
    assert!(
        wait_ready(&reactor, token, Duration::from_secs(2)),
        "a cleared registration must re-arm on the next edge"
    );

    reactor.deregister(read_fd, token).expect("deregister");
    // SAFETY: both fds are owned by this test and not used again.
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}

#[test]
#[serial_test::serial]
fn readiness_reports_actual_flags_not_a_collapsed_mask() {
    let reactor = Reactor::get().expect("shared reactor must initialise");
    let (read_fd, write_fd) = nonblocking_pipe().expect("pipe");
    let token = Token(0xF43_0003);

    reactor
        .register(read_fd, token, Interest::READABLE)
        .expect("register read end");

    write_byte(write_fd);
    assert!(
        wait_flag(&reactor, token, Ready::READABLE, Duration::from_secs(2)),
        "pipe must latch READABLE"
    );

    let ready = reactor.readiness(token);
    assert!(
        !ready.is_writable(),
        "a read-interest registration must never report WRITABLE; got {ready}. \
         Collapsing readiness to READABLE|WRITABLE hides READ_CLOSED and ERROR"
    );

    // Closing the write end is a fresh edge carrying EPOLLHUP.
    // SAFETY: `write_fd` is owned by this test and not used again.
    unsafe { libc::close(write_fd) };

    assert!(
        wait_flag(&reactor, token, Ready::READ_CLOSED, Duration::from_secs(2)),
        "closing the write end must surface READ_CLOSED, not just READABLE — \
         got {}",
        reactor.readiness(token)
    );

    reactor.deregister(read_fd, token).expect("deregister");
    // SAFETY: `read_fd` is owned by this test and not used again.
    unsafe { libc::close(read_fd) };
}

/// Count the process's open file descriptors.
///
/// `/proc/self/fd` is itself read through an open fd, so the returned count is
/// only meaningful as a *delta* between two calls made the same way.
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .expect("procfs must be mounted")
        .count()
}

#[test]
#[serial_test::serial]
fn registering_many_fds_adds_no_selector_fds() {
    let reactor = Reactor::get().expect("shared reactor must initialise");

    const N: usize = 64;
    let mut pipes = Vec::with_capacity(N);
    for _ in 0..N {
        pipes.push(nonblocking_pipe().expect("pipe"));
    }

    // Baseline *after* the pipes exist, so the delta isolates registration cost.
    let before = open_fd_count();

    for (i, (read_fd, _)) in pipes.iter().enumerate() {
        reactor
            .register(*read_fd, Token(0xF43_1000 + i), Interest::READABLE)
            .expect("register");
    }

    let after = open_fd_count();
    assert_eq!(
        after, before,
        "registering {N} fds into the shared reactor allocated {} new fds; a \
         shared reactor must add none — one private epoll fd per registration is \
         exactly what Decision 14 §Scope 3 removes",
        after.saturating_sub(before)
    );

    for (i, (read_fd, write_fd)) in pipes.iter().enumerate() {
        reactor.deregister(*read_fd, Token(0xF43_1000 + i)).expect("deregister");
        // SAFETY: fds are owned by this test and not used again.
        unsafe {
            libc::close(*read_fd);
            libc::close(*write_fd);
        }
    }
}

#[test]
#[serial_test::serial]
fn unregistered_token_is_never_ready() {
    let reactor = Reactor::get().expect("shared reactor must initialise");
    let token = Token(0xF43_00FF);

    assert!(
        !reactor.is_ready(token),
        "a token that was never registered must not report ready"
    );
    assert_eq!(
        reactor.readiness(token),
        Ready::EMPTY,
        "an unregistered token has no readiness"
    );

    // Clearing an unknown token is a no-op, not a panic.
    reactor.clear(token, Ready::ALL);
}
