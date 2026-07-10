//! io_uring completion-mode read path (F43 — Decision 14 F4).
//!
//! WHY: completion mode changes who reads the socket. The kernel fills a buffer
//! from a registered pool and hands us the buffer id; the transport pops bytes
//! instead of calling `read(2)`. Everything about that is invisible from the
//! outside except through tests: the bytes must be right, the buffer must go
//! back to the pool when dropped, EOF and errors must still surface, and
//! non-socket fds must keep working on the poll path.
//!
//! WHAT: drives `uring_completion::Selector` directly over socketpairs and
//! pipes, asserting the delivery, the recycle protocol, and the fallbacks.
//!
//! HOW: a small buffer ring makes starvation reachable. Each test owns its
//! selector, so there is no shared-singleton coupling.
//!
//! The zero-`read()` acceptance criterion lives in
//! `uring_completion_syscall_tests.rs`, which counts syscalls with `ptrace`.

#![cfg(all(target_os = "linux", feature = "uring"))]

use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use foundation_nativeapis::native::poll::probe;
use foundation_nativeapis::native::poll::sys::unix::selector::uring_completion::{
    Completion, Selector,
};
use foundation_nativeapis::{Events, Interest, Token};

/// Skip a test when the running kernel lacks the completion tier.
///
/// Decision 14 makes this a documented fallback, not a failure: below the
/// 5.19/6.0 matrix the ladder selects readiness mode. Version numbers are never
/// consulted — the functional probe is the gate.
macro_rules! require_completion_tier {
    () => {
        match probe::probe() {
            Ok(caps) if caps.supports_completion() => {}
            Ok(caps) => {
                eprintln!("skipping: kernel lacks the completion tier ({caps})");
                return;
            }
            Err(e) => {
                eprintln!("skipping: io_uring unavailable ({e})");
                return;
            }
        }
    };
}

fn socketpair() -> (RawFd, RawFd) {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-element array for socketpair to fill.
    let rc = unsafe {
        libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0, fds.as_mut_ptr())
    };
    assert!(rc == 0, "socketpair: {}", io::Error::last_os_error());
    (fds[0], fds[1])
}

fn nonblocking_pipe() -> (RawFd, RawFd) {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe2 to fill.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    assert!(rc == 0, "pipe2: {}", io::Error::last_os_error());
    (fds[0], fds[1])
}

fn write_all(fd: RawFd, data: &[u8]) {
    // SAFETY: writing `data.len()` bytes from a valid slice to an owned fd.
    let n = unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) };
    assert_eq!(n, data.len() as isize, "write: {}", io::Error::last_os_error());
}

fn close(fd: RawFd) {
    // SAFETY: `fd` is owned by the caller and not used again.
    unsafe { libc::close(fd) };
}

/// Poll until `token` yields completions, or the budget expires.
fn collect_completions(sel: &Selector, token: Token, budget: Duration) -> Vec<Completion> {
    let deadline = Instant::now() + budget;
    let mut events = Events::with_capacity(16);
    loop {
        events.clear();
        sel.poll(&mut events, Some(Duration::from_millis(25))).expect("poll");
        let completions = sel.take_completions(token);
        if !completions.is_empty() {
            return completions;
        }
        if Instant::now() >= deadline {
            return Vec::new();
        }
    }
}

#[test]
fn kernel_delivers_socket_bytes_into_a_provided_buffer() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    write_all(b, b"hello completion");

    let completions = collect_completions(&sel, token, Duration::from_secs(2));
    assert_eq!(completions.len(), 1, "expected exactly one completion");

    match &completions[0] {
        Completion::Data(buf) => {
            assert_eq!(
                &buf[..],
                b"hello completion",
                "the kernel-filled buffer must carry the bytes the peer sent"
            );
        }
        other => panic!("expected Data, got {other:?}"),
    }

    sel.deregister_fd(a).expect("deregister");
    close(a);
    close(b);
}

#[test]
fn dropping_a_provided_buf_returns_it_to_the_pool() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    write_all(b, b"recycle me");

    let before = sel.bufring().recycle_count();
    let completions = collect_completions(&sel, token, Duration::from_secs(2));
    assert_eq!(completions.len(), 1);

    assert_eq!(
        sel.bufring().recycle_count(),
        before,
        "a buffer still held by a ProvidedBuf must not be back in the pool"
    );

    drop(completions);

    assert_eq!(
        sel.bufring().recycle_count(),
        before + 1,
        "dropping the ProvidedBuf must republish its buffer, or the pool drains \
         and the kernel starts answering -ENOBUFS"
    );

    sel.deregister_fd(a).expect("deregister");
    close(a);
    close(b);
}

#[test]
fn multishot_recv_delivers_successive_writes() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");

    for i in 0..4u8 {
        write_all(b, &[b'a' + i]);
        let completions = collect_completions(&sel, token, Duration::from_secs(2));
        assert_eq!(
            completions.len(),
            1,
            "write {i} produced {} completions; multishot must stay armed",
            completions.len()
        );
        match &completions[0] {
            Completion::Data(buf) => assert_eq!(&buf[..], &[b'a' + i]),
            other => panic!("write {i}: expected Data, got {other:?}"),
        }
    }

    sel.deregister_fd(a).expect("deregister");
    close(a);
    close(b);
}

#[test]
fn peer_close_surfaces_as_eof() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    close(b);

    let completions = collect_completions(&sel, token, Duration::from_secs(2));
    assert_eq!(completions.len(), 1, "closing the peer must produce one completion");
    assert!(
        matches!(completions[0], Completion::Eof),
        "a closed peer is EOF, not data or an error; got {:?}",
        completions[0]
    );

    sel.deregister_fd(a).expect("deregister");
    close(a);
}

#[test]
fn bytes_still_arrive_when_the_peer_closes_immediately_after_writing() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    write_all(b, b"last words");
    close(b);

    // Data and EOF may arrive in one drain or two; what must never happen is
    // losing the bytes because the close overtook them.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut all = Vec::new();
    let mut events = Events::with_capacity(16);
    while Instant::now() < deadline {
        events.clear();
        sel.poll(&mut events, Some(Duration::from_millis(25))).expect("poll");
        all.extend(sel.take_completions(token));
        if all.iter().any(|c| matches!(c, Completion::Eof)) {
            break;
        }
    }

    let data: Vec<u8> = all
        .iter()
        .filter_map(|c| match c {
            Completion::Data(buf) => Some(buf.to_vec()),
            _ => None,
        })
        .flatten()
        .collect();

    assert_eq!(data, b"last words", "the bytes must survive an immediate close");
    assert!(
        all.iter().any(|c| matches!(c, Completion::Eof)),
        "EOF must still be reported after the final data"
    );

    sel.deregister_fd(a).expect("deregister");
    close(a);
}

#[test]
fn non_socket_fds_fall_back_to_the_poll_path() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (r, w) = nonblocking_pipe();
    let token = Token(7);

    // A pipe is not a socket: RECV would fail with -ENOTSOCK, so the selector
    // must register it with multishot POLL_ADD instead.
    sel.register_fd(r, token, Interest::READABLE).expect("register pipe");
    write_all(w, b"x");

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_readable = false;
    let mut events = Events::with_capacity(16);
    while Instant::now() < deadline && !saw_readable {
        events.clear();
        sel.poll(&mut events, Some(Duration::from_millis(25))).expect("poll");
        saw_readable = events.iter().any(|e| e.token() == token && e.is_readable());
    }

    assert!(saw_readable, "a pipe must still report readable in completion mode");
    assert!(
        sel.take_completions(token).is_empty(),
        "a poll-path fd has no kernel-filled buffers; the caller must read() it"
    );

    sel.deregister_fd(r).expect("deregister");
    close(r);
    close(w);
}

#[test]
fn a_starved_pool_recovers_once_buffers_are_returned() {
    require_completion_tier!();

    // One buffer, so the second inbound message finds the pool empty.
    let sel = Selector::with_ring(1, 64).expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");

    write_all(b, b"first");
    let first = collect_completions(&sel, token, Duration::from_secs(2));
    assert_eq!(first.len(), 1, "first message consumes the only buffer");

    // Hold the buffer: the pool is now empty. The next message starves the
    // recv, which ends the multishot registration with -ENOBUFS.
    write_all(b, b"second");
    let mut events = Events::with_capacity(16);
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        events.clear();
        sel.poll(&mut events, Some(Duration::from_millis(25))).expect("poll");
        assert!(
            sel.take_completions(token).is_empty(),
            "no buffer is available, so nothing can be delivered"
        );
    }

    assert_eq!(
        sel.starved_tokens(),
        1,
        "the empty pool must have parked this token via -ENOBUFS; if it did not, \
         this test is not exercising the starvation path at all"
    );

    // Release the buffer. The selector must notice and re-arm.
    drop(first);

    let second = collect_completions(&sel, token, Duration::from_secs(3));
    assert_eq!(
        second.len(),
        1,
        "after a buffer is recycled the starved recv must re-arm and deliver"
    );
    match &second[0] {
        Completion::Data(buf) => assert_eq!(&buf[..], b"second"),
        other => panic!("expected Data, got {other:?}"),
    }
    assert_eq!(sel.starved_tokens(), 0, "the token must no longer be parked");

    sel.deregister_fd(a).expect("deregister");
    close(a);
    close(b);
}

/// The evidence for Decision 14 OQ#14.1's committed re-evaluation.
///
/// OQ#14.1 deferred per-worker rings, with a trigger: *"when completion mode
/// lands (14-F4), per-op submissions make SQ contention real — revisit
/// per-worker rings then."* That prediction assumed the completion path submits
/// an SQE per read. Multishot `RECV` does not: one SQE arms an fd for its whole
/// lifetime, and the kernel keeps delivering.
///
/// If this test ever fails — if submissions start growing with reads — the
/// premise of OQ#14.1's deferral has returned and per-worker rings are back on
/// the table.
#[test]
fn submissions_are_per_registration_not_per_read() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    let after_register = sel.submissions();
    assert_eq!(after_register, 1, "arming a socket takes exactly one SQE");

    const MESSAGES: usize = 64;
    for i in 0..MESSAGES {
        let msg = format!("message {i}");
        write_all(b, msg.as_bytes());
        let completions = collect_completions(&sel, token, Duration::from_secs(2));
        assert_eq!(completions.len(), 1, "message {i} must be delivered");
        drop(completions);
    }

    let after_reads = sel.submissions();
    assert_eq!(
        after_reads, after_register,
        "{MESSAGES} reads pushed {} extra SQE(s). Multishot RECV is supposed to arm \
         once per fd; per-read submission would make the SQ lock a real contention \
         point and would re-open Decision 14 OQ#14.1's per-worker-ring question",
        after_reads - after_register
    );

    sel.deregister_fd(a).expect("deregister");
    close(a);
    close(b);
}

#[test]
fn deregistered_socket_stops_delivering() {
    require_completion_tier!();

    let sel = Selector::new().expect("completion selector");
    let (a, b) = socketpair();
    let token = Token(1);

    sel.register_fd(a, token, Interest::READABLE).expect("register socket");
    sel.deregister_fd(a).expect("deregister");

    write_all(b, b"ignored");
    let completions = collect_completions(&sel, token, Duration::from_millis(300));
    assert!(
        completions.is_empty(),
        "a deregistered socket must deliver nothing, got {completions:?}"
    );

    close(a);
    close(b);
}
