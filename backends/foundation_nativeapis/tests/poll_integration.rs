/// Integration test: Poll selector correctness.
///
/// Tests the extracted mio-style poll layer:
/// - Register fd, poll returns event when data arrives
/// - Deregister fd, poll returns no events
/// - Waker wakes from another thread
/// - Multiple tokens return correct tokens

use foundation_nativeapis::poll::{Events, Interest, Poll, SourceFd, Token, Waker};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Create a pipe (reader, writer), both nonblocking.
fn make_pipe() -> (OwnedFd, OwnedFd) {
    let (r, w) = nix::unistd::pipe().expect("pipe creation failed");
    nix::fcntl::fcntl(r.as_raw_fd(), nix::fcntl::FcntlArg::F_SETFL(
        nix::fcntl::OFlag::O_NONBLOCK
    )).expect("fcntl O_NONBLOCK failed");
    nix::fcntl::fcntl(w.as_raw_fd(), nix::fcntl::FcntlArg::F_SETFL(
        nix::fcntl::OFlag::O_NONBLOCK
    )).expect("fcntl O_NONBLOCK failed");
    (r, w)  // nix::pipe already returns OwnedFd
}

/// Write to a raw fd.
fn write_fd(fd: std::os::unix::io::RawFd, data: &[u8]) -> std::io::Result<()> {
    let n = unsafe {
        libc::write(fd, data.as_ptr() as *const libc::c_void, data.len())
    };
    if n < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
}

/// Test: Register fd, poll returns event when data arrives
#[test]
fn poll_returns_event_when_data_arrives() {
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();
    let (reader, writer) = make_pipe();

    registry
        .register(&mut SourceFd(reader.as_raw_fd()), Token(42), Interest::READABLE)
        .expect("register failed");

    // Poll with short timeout — no data yet
    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(10)))
        .expect("poll failed");
    assert!(events.is_empty(), "expected no events before write");

    // Write data
    write_fd(writer.as_raw_fd(), b"hello").expect("write failed");

    // Poll again — event should arrive
    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");
    assert_eq!(events.len(), 1, "expected 1 event");
    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), Token(42));
    assert!(event.is_readable());
}

/// Test: Deregister fd, poll returns no events
#[test]
fn poll_returns_no_events_after_deregister() {
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();
    let (reader, writer) = make_pipe();

    registry
        .register(&mut SourceFd(reader.as_raw_fd()), Token(7), Interest::READABLE)
        .expect("register failed");

    // Deregister
    registry
        .deregister(&mut SourceFd(reader.as_raw_fd()))
        .expect("deregister failed");

    // Write data — should not trigger an event
    write_fd(writer.as_raw_fd(), b"hello").expect("write failed");

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(50)))
        .expect("poll failed");
    assert!(
        events.is_empty(),
        "expected no events after deregister, got {} events",
        events.len()
    );
}

/// Test: Waker wakes from another thread
#[test]
fn waker_wakes_from_another_thread() {
    let poll = Poll::new().expect("failed to create poll");
    let waker = Arc::new(Waker::new(&poll.registry(), Token(999)).expect("waker create failed"));

    let waker_clone = waker.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        waker_clone.wake().expect("wake failed");
    });

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_secs(5)))
        .expect("poll failed");

    // Should have gotten the waker event
    let tokens: Vec<_> = events.iter().map(|e| e.token()).collect();
    assert!(
        tokens.contains(&Token(999)),
        "expected waker event, got: {:?}",
        tokens
    );

    handle.join().expect("thread panicked");
}

/// Test: Multiple fds with different tokens
#[test]
fn poll_returns_correct_tokens_for_multiple_fds() {
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();
    let (r1, w1) = make_pipe();
    let (r2, w2) = make_pipe();

    registry
        .register(&mut SourceFd(r1.as_raw_fd()), Token(1), Interest::READABLE)
        .expect("register r1 failed");
    registry
        .register(&mut SourceFd(r2.as_raw_fd()), Token(2), Interest::READABLE)
        .expect("register r2 failed");

    // Write to pipe 2 only
    write_fd(w2.as_raw_fd(), b"data for pipe 2").expect("write failed");

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");

    assert_eq!(events.len(), 1);
    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), Token(2));
    assert!(event.is_readable());

    // Now write to pipe 1
    write_fd(w1.as_raw_fd(), b"data for pipe 1").expect("write failed");

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");

    assert_eq!(events.len(), 1);
    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), Token(1));
}

/// Test: Poll with zero timeout returns immediately
#[test]
fn poll_zero_timeout_returns_immediately() {
    let poll = Poll::new().expect("failed to create poll");

    let mut events = Events::with_capacity(16);
    let start = std::time::Instant::now();
    poll.poll(&mut events, Some(Duration::ZERO))
        .expect("poll failed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(50),
        "poll with zero timeout took too long: {:?}",
        elapsed
    );
    assert!(events.is_empty());
}

/// Test: Interest::WRITABLE returns writable events
#[test]
fn poll_writable_interest() {
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();
    let (_reader, writer) = make_pipe();

    registry
        .register(&mut SourceFd(writer.as_raw_fd()), Token(10), Interest::WRITABLE)
        .expect("register failed");

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");

    // Pipe write end should be writable
    let is_writable = events.iter().any(|e| e.token() == Token(10) && e.is_writable());
    assert!(
        is_writable,
        "expected writable event for token 10"
    );
}
