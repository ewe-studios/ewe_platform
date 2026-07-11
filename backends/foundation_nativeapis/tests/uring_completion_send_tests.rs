//! io_uring completion-mode write path — `IORING_OP_SEND` (F49 — Decision 14 F4).
//!
//! WHY: the SEND half of completion mode. A transport hands the kernel an owned
//! buffer and the bytes go on the wire with no `write(2)`; the completion returns
//! the byte count and recycles the buffer. Like the RECV side, all of this is
//! invisible except through tests: the peer must receive exactly the submitted
//! bytes, a write larger than a pool buffer must report a short send, and the
//! pool must recycle so many sends reuse a few buffers.
//!
//! WHAT: drives `uring_completion::Selector::submit_send` directly over
//! socketpairs, asserting delivery, the short-send contract, and ordering.
//!
//! HOW: each test owns its selector. The zero-`write()` acceptance criterion
//! lives in `uring_completion_syscall_tests.rs`, which counts syscalls with
//! `ptrace`.

#![cfg(all(target_os = "linux", feature = "uring"))]

use std::io;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

use foundation_nativeapis::native::poll::probe;
use foundation_nativeapis::native::poll::sys::unix::selector::uring_completion::{
    SendCompletion, Selector,
};
use foundation_nativeapis::{Events, Token};

/// Skip when the running kernel lacks the completion tier (documented fallback).
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
    // Non-blocking so a peer `read` with nothing pending returns immediately
    // rather than blocking the test thread.
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
    (fds[0], fds[1])
}

fn read_available(fd: RawFd, buf: &mut [u8]) -> usize {
    // SAFETY: reading into a valid writable slice from an owned fd.
    let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
    if n < 0 {
        0
    } else {
        n as usize
    }
}

fn close(fd: RawFd) {
    // SAFETY: `fd` is owned by the caller and not used again.
    unsafe { libc::close(fd) };
}

/// Poll until `token` yields at least one finished send, or the budget expires.
fn poll_sends(sel: &Selector, token: Token, budget: Duration) -> Vec<SendCompletion> {
    let deadline = Instant::now() + budget;
    let mut events = Events::with_capacity(16);
    loop {
        events.clear();
        sel.poll(&mut events, Some(Duration::from_millis(25))).expect("poll");
        let sends = sel.take_send_completions(token);
        if !sends.is_empty() {
            return sends;
        }
        if Instant::now() >= deadline {
            return Vec::new();
        }
    }
}

#[test]
fn send_places_bytes_on_the_peer_socket() {
    require_completion_tier!();
    let (a, b) = socketpair();
    let sel = Selector::new().expect("completion selector");
    let token = Token(1);
    let payload = b"hello over IORING_OP_SEND";

    let taken = sel.submit_send(token, a, payload).expect("submit_send");
    assert_eq!(taken, payload.len(), "small write is taken whole");

    let sends = poll_sends(&sel, token, Duration::from_secs(2));
    assert_eq!(sends.len(), 1, "exactly one send completion");
    match &sends[0] {
        SendCompletion::Written(n) => assert_eq!(*n, payload.len()),
        other => panic!("expected Written, got {other:?}"),
    }

    // The peer receives exactly those bytes.
    let mut buf = [0u8; 64];
    let got = read_available(b, &mut buf);
    assert_eq!(&buf[..got], payload);

    // The send is finished — nothing is left in flight, so a flush would succeed.
    assert!(!sel.has_pending_sends(token), "no sends left in flight");

    close(a);
    close(b);
}

#[test]
fn a_write_larger_than_the_pool_buffer_is_a_short_send() {
    require_completion_tier!();
    let (a, b) = socketpair();
    // 8-byte pool buffers force a short send of a 16-byte payload.
    let sel = Selector::with_ring(4, 8).expect("completion selector");
    let token = Token(7);
    let payload = b"0123456789ABCDEF"; // 16 bytes

    let taken = sel.submit_send(token, a, payload).expect("submit_send");
    assert_eq!(taken, 8, "capped at the pool buffer size — caller resubmits the rest");

    let sends = poll_sends(&sel, token, Duration::from_secs(2));
    assert!(matches!(sends.as_slice(), [SendCompletion::Written(8)]));

    let mut buf = [0u8; 64];
    let got = read_available(b, &mut buf);
    assert_eq!(&buf[..got], &payload[..8]);

    close(a);
    close(b);
}

#[test]
fn successive_sends_deliver_in_order() {
    require_completion_tier!();
    let (a, b) = socketpair();
    let sel = Selector::new().expect("completion selector");
    let token = Token(2);

    for msg in [b"one".as_slice(), b"two", b"three"] {
        let taken = sel.submit_send(token, a, msg).expect("submit_send");
        assert_eq!(taken, msg.len());
        let sends = poll_sends(&sel, token, Duration::from_secs(2));
        assert!(matches!(sends.as_slice(), [SendCompletion::Written(n)] if *n == msg.len()));
    }

    let mut buf = [0u8; 64];
    let got = read_available(b, &mut buf);
    assert_eq!(&buf[..got], b"onetwothree", "bytes arrive in submission order");

    close(a);
    close(b);
}

#[test]
fn a_small_pool_recycles_across_many_sends() {
    require_completion_tier!();
    let (a, b) = socketpair();
    // Only two buffers: 30 sends can only succeed if each recycles on completion.
    let sel = Selector::with_ring(2, 64).expect("completion selector");
    let token = Token(4);

    let mut received = Vec::new();
    for i in 0..30u8 {
        let data = [i; 4];
        // Retry while the pool is momentarily exhausted, draining completions to
        // recycle buffers — the WouldBlock/park contract, driven synchronously.
        loop {
            match sel.submit_send(token, a, &data) {
                Ok(n) => {
                    assert_eq!(n, 4);
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    poll_sends(&sel, token, Duration::from_millis(200));
                }
                Err(e) => panic!("submit_send: {e}"),
            }
        }
        poll_sends(&sel, token, Duration::from_millis(50));
        let mut buf = [0u8; 256];
        let got = read_available(b, &mut buf);
        received.extend_from_slice(&buf[..got]);
    }
    // Drain any trailing completions + bytes.
    poll_sends(&sel, token, Duration::from_millis(200));
    let mut buf = [0u8; 256];
    let got = read_available(b, &mut buf);
    received.extend_from_slice(&buf[..got]);

    assert_eq!(received.len(), 30 * 4, "every submitted byte reached the peer");

    close(a);
    close(b);
}
