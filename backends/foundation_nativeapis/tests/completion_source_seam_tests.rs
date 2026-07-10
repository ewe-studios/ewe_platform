//! The `CompletionSource` seam through the shared reactor (F43 — D14 F4).
//!
//! WHY: `uring_completion_tests.rs` drives the selector directly. This drives
//! the seam a transport actually sees: a `RegisteredFd` on the process reactor,
//! parked on `EventReadiness`, woken, and drained through `CompletionSource`.
//! Decision 14's claim is that these two traits sit side by side and the task
//! cannot tell which mode it is in — that only holds if both work on the same
//! object.
//!
//! WHAT: registration → wake → `take_completions` → buffer recycled on drop,
//! plus the branch a transport writes: sockets are completion sources, pipes on
//! the same reactor are not.
//!
//! HOW: the reactor is a process-wide singleton, so these tests share whatever
//! backend the F42 ladder picked. Each asserts against `Reactor::backend()`
//! rather than assuming, and skips when the kernel cannot reach completion mode
//! — which is the documented fallback, not a failure.

#![cfg(all(target_os = "linux", feature = "uring", feature = "fd"))]

use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::time::{Duration, Instant};

use foundation_core::valtron::EventReadiness;
use foundation_nativeapis::native::fd::{Completion, CompletionSource, Reactor, RegisteredFd};
use foundation_nativeapis::native::poll::Backend;
use foundation_nativeapis::{Interest, Token};

/// Skip unless the shared reactor actually landed on completion mode.
macro_rules! require_completion_backend {
    ($reactor:expr) => {
        if $reactor.backend() != Backend::UringCompletion {
            eprintln!(
                "skipping: reactor is on {} — completion mode unavailable on this kernel",
                $reactor.backend()
            );
            return;
        }
    };
}

fn socketpair() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-element array for socketpair to fill.
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
            fds.as_mut_ptr(),
        )
    };
    assert!(rc == 0, "socketpair: {}", io::Error::last_os_error());
    // SAFETY: both are fresh, valid, owned descriptors.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn nonblocking_pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe2 to fill.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    assert!(rc == 0, "pipe2: {}", io::Error::last_os_error());
    // SAFETY: both are fresh, valid, owned descriptors.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn write_all(fd: &OwnedFd, data: &[u8]) {
    use std::os::fd::AsRawFd;
    // SAFETY: writing `data.len()` bytes from a valid slice to an owned fd.
    let n = unsafe { libc::write(fd.as_raw_fd(), data.as_ptr().cast(), data.len()) };
    assert_eq!(n, data.len() as isize, "write: {}", io::Error::last_os_error());
}

/// Wait for the reactor's drain thread to latch readiness for `fd`.
fn wait_ready(fd: &impl EventReadiness, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if fd.is_ready(None) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn a_socket_is_a_completion_source_and_a_pipe_is_not() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_01), Interest::READABLE)
        .expect("register socket");

    let (r, w) = nonblocking_pipe();
    let pipe = RegisteredFd::with_interest(r, reactor.registry(), Token(0xC0DE_02), Interest::READABLE)
        .expect("register pipe");

    assert!(
        sock.is_completion_source(),
        "a socket on a completion-mode reactor delivers bytes as completions"
    );
    assert!(
        !pipe.is_completion_source(),
        "a pipe is not a socket: RECV would fail with -ENOTSOCK, so it stays on the \
         poll path and its bytes still need a read(2). A transport that skipped \
         read() here would hang forever"
    );

    drop(b);
    drop(w);
}

#[test]
fn parked_task_wakes_and_drains_kernel_filled_bytes() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_10), Interest::READABLE)
        .expect("register socket");

    assert!(!sock.is_ready(None), "an idle socket must not be ready");
    assert!(!sock.has_completions(), "nothing has been sent yet");

    write_all(&b, b"seam bytes");

    assert!(
        wait_ready(&sock, Duration::from_secs(2)),
        "the reactor must wake a task parked on Depends(RegisteredFd) in completion \
         mode exactly as it does in readiness mode"
    );

    let completions = sock.take_completions();
    assert_eq!(completions.len(), 1, "expected one completion, got {completions:?}");
    match &completions[0] {
        Completion::Data(buf) => assert_eq!(&buf[..], b"seam bytes"),
        other => panic!("expected Data, got {other:?}"),
    }

    drop(b);
}

#[test]
fn draining_the_inbox_clears_readiness_so_the_task_parks_again() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_20), Interest::READABLE)
        .expect("register socket");

    write_all(&b, b"one");
    assert!(wait_ready(&sock, Duration::from_secs(2)), "first write must wake");

    let first = sock.take_completions();
    assert_eq!(first.len(), 1);
    drop(first);

    assert!(
        !sock.has_completions(),
        "the inbox is empty after draining it"
    );
    assert!(
        !sock.is_ready(None),
        "draining the inbox is completion mode's read-until-WouldBlock: readiness \
         must clear, or the woken task spins instead of parking"
    );

    // A fresh arrival must wake it again.
    write_all(&b, b"two");
    assert!(
        wait_ready(&sock, Duration::from_secs(2)),
        "a cleared completion source must re-arm on the next delivery"
    );
    let second = sock.take_completions();
    assert_eq!(second.len(), 1);
    match &second[0] {
        Completion::Data(buf) => assert_eq!(&buf[..], b"two"),
        other => panic!("expected Data, got {other:?}"),
    }

    drop(b);
}

#[test]
fn buffers_return_to_the_pool_when_completions_are_dropped() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_30), Interest::READABLE)
        .expect("register socket");

    // Round-trip more messages than a naive implementation could serve if it
    // never recycled: a leak shows up as a stall once the pool empties.
    for i in 0..64u32 {
        let msg = format!("message {i}");
        write_all(&b, msg.as_bytes());
        assert!(
            wait_ready(&sock, Duration::from_secs(2)),
            "message {i} never arrived — the buffer pool has probably leaked"
        );
        let completions = sock.take_completions();
        assert_eq!(completions.len(), 1, "message {i}");
        match &completions[0] {
            Completion::Data(buf) => assert_eq!(&buf[..], msg.as_bytes(), "message {i}"),
            other => panic!("message {i}: expected Data, got {other:?}"),
        }
        // Dropping here returns the buffer; without it the pool drains.
        drop(completions);
    }

    drop(b);
}

#[test]
fn read_bytes_serves_a_completion_across_several_small_reads() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_50), Interest::READABLE)
        .expect("register socket");

    let payload: Vec<u8> = (0..100u8).collect();
    write_all(&b, &payload);
    assert!(wait_ready(&sock, Duration::from_secs(2)), "payload must arrive");

    // A `read`-shaped caller with a buffer smaller than the completion must see
    // every byte, in order, across successive calls — the kernel's buffer is
    // consumed incrementally and only recycled once drained.
    let mut got = Vec::new();
    let mut chunk = [0u8; 16];
    loop {
        match sock.read_bytes(&mut chunk) {
            Ok(0) => break,
            Ok(n) => got.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("read_bytes: {e}"),
        }
    }

    assert_eq!(got, payload, "partial reads must reassemble the payload exactly");

    drop(b);
}

#[test]
fn read_bytes_reports_would_block_when_the_inbox_is_empty() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_60), Interest::READABLE)
        .expect("register socket");

    let mut chunk = [0u8; 16];
    let err = sock.read_bytes(&mut chunk).expect_err("an idle socket has nothing to read");
    assert_eq!(
        err.kind(),
        io::ErrorKind::WouldBlock,
        "an empty inbox must look exactly like an empty socket, so a transport's \
         read loop is identical on both backends"
    );

    drop(b);
}

#[test]
fn read_bytes_reports_eof_after_the_peer_closes() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_70), Interest::READABLE)
        .expect("register socket");

    write_all(&b, b"bye");
    drop(b);
    assert!(wait_ready(&sock, Duration::from_secs(2)), "data or close must wake");

    let mut chunk = [0u8; 16];
    let mut got = Vec::new();
    let mut saw_eof = false;
    for _ in 0..8 {
        match sock.read_bytes(&mut chunk) {
            Ok(0) => {
                saw_eof = true;
                break;
            }
            Ok(n) => got.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("read_bytes: {e}"),
        }
    }

    assert_eq!(got, b"bye", "the final bytes must precede EOF");
    assert!(saw_eof, "the peer closed, so read_bytes must eventually return Ok(0)");

    // EOF latches: a second read must not go back to WouldBlock, or a transport
    // would park forever on a dead socket.
    assert_eq!(
        sock.read_bytes(&mut chunk).expect("read after EOF"),
        0,
        "EOF must latch"
    );
}

#[test]
fn peer_close_surfaces_as_eof_through_the_seam() {
    let reactor = Reactor::get().expect("reactor");
    require_completion_backend!(reactor);

    let (a, b) = socketpair();
    let sock = RegisteredFd::with_interest(a, reactor.registry(), Token(0xC0DE_40), Interest::READABLE)
        .expect("register socket");

    drop(b);

    assert!(wait_ready(&sock, Duration::from_secs(2)), "a closed peer must wake the task");

    let completions = sock.take_completions();
    assert!(
        completions.iter().any(|c| matches!(c, Completion::Eof)),
        "a closed peer is EOF through the seam, not silence; got {completions:?}"
    );
}
