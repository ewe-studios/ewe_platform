//! F50 B2: the reactor's event-generation wait primitive.
//!
//! WHY: a proxy splice is a raw OS thread, not a valtron task, so it cannot park
//! via `TaskStatus::Depends`. `Reactor::wait_for_events` lets it block until the
//! reactor latches new readiness — the enabler for a readiness-aware splice that
//! does not busy-poll. `is_ready` is a non-blocking latch query, so this is the
//! only way a non-task thread can block on reactor activity.
//!
//! WHAT: a registered fd, a delayed write from another thread, and the assertion
//! that a thread parked on `wait_for_events` wakes on the resulting event (before
//! the timeout), plus the lost-wakeup guard (a stale generation returns at once).

#![cfg(target_os = "linux")]

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use foundation_nativeapis::native::fd::{Reactor, RegisteredFd};
use foundation_nativeapis::Token;

fn nonblocking_pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe2 to fill.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    assert!(rc == 0, "pipe2 failed");
    // SAFETY: pipe2 returned two fresh, owned descriptors.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn write_byte(fd: RawFd) {
    let b = [1u8];
    // SAFETY: writing 1 byte from a valid buffer to an owned fd.
    let n = unsafe { libc::write(fd, b.as_ptr().cast(), 1) };
    assert_eq!(n, 1, "write to pipe failed");
}

#[test]
fn wait_for_events_wakes_when_a_registered_fd_becomes_readable() {
    let reactor = Reactor::get().expect("shared reactor");
    let (reader, writer) = nonblocking_pipe();
    let _reg = RegisteredFd::new(reader, reactor.registry(), Token(0x5002_0001))
        .expect("register reader");

    // Snapshot the generation before the event.
    let gen = reactor.events_generation();

    // Write from another thread after a delay, so the wait is genuinely parked.
    let writer_fd = writer.as_raw_fd();
    let held = writer; // keep the write end open for the thread's lifetime
    let h = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        write_byte(writer_fd);
        drop(held);
    });

    let start = Instant::now();
    let new_gen = reactor.wait_for_events(gen, Duration::from_secs(3));
    let elapsed = start.elapsed();
    h.join().ok();

    assert_ne!(new_gen, gen, "generation must advance when the fd becomes readable");
    assert!(
        elapsed < Duration::from_secs(3),
        "woke on the event ({elapsed:?}), not by the timeout"
    );
}

#[test]
fn wait_for_events_returns_immediately_for_a_stale_generation() {
    let reactor = Reactor::get().expect("shared reactor");
    let (reader, writer) = nonblocking_pipe();
    let reg = Arc::new(
        RegisteredFd::new(reader, reactor.registry(), Token(0x5002_0002)).expect("register"),
    );
    let _ = &reg;

    let gen = reactor.events_generation();

    // Cause an event and let the drain thread latch it (advancing the generation).
    write_byte(writer.as_raw_fd());
    let deadline = Instant::now() + Duration::from_secs(2);
    while reactor.events_generation() == gen && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert_ne!(reactor.events_generation(), gen, "drain thread advanced the generation");

    // Waiting on the now-stale `gen` must return at once, not block for the timeout
    // — the lost-wakeup guard.
    let start = Instant::now();
    let observed = reactor.wait_for_events(gen, Duration::from_secs(5));
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "a stale generation returns immediately"
    );
    assert_ne!(observed, gen);
    drop(writer);
}
