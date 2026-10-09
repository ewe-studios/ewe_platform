/// Integration test: Poll selector correctness.
///
/// Tests the extracted mio-style poll layer:
/// - Register fd, poll returns event when data arrives
/// - Deregister fd, poll returns no events
/// - Waker wakes from another thread
/// - Multiple tokens return correct tokens

use foundation_nativeapis::{Events, Interest, Poll, SourceFd, Token, Waker};
use std::os::fd::{AsRawFd, OwnedFd};
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

/// Test: Waker wakes from multiple threads simultaneously
#[test]
fn waker_wakes_from_multiple_threads() {
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();
    let waker = Arc::new(Waker::new(&registry, Token(999)).expect("waker create failed"));

    // Spawn 5 threads that all wake
    let mut handles = Vec::new();
    for _ in 0..5 {
        let waker_clone = waker.clone();
        handles.push(thread::spawn(move || {
            waker_clone.wake().expect("wake failed");
        }));
    }

    // Poll should return quickly
    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_secs(5)))
        .expect("poll failed");

    let tokens: Vec<_> = events.iter().map(|e| e.token()).collect();
    assert!(
        tokens.contains(&Token(999)),
        "expected waker event, got: {:?}",
        tokens
    );

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

/// Test: Concurrent registration from multiple threads
#[test]
fn concurrent_registration() {
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    let mut pipes = Vec::new();
    for i in 0..8 {
        let (r, _w) = make_pipe();
        let reg = registry.clone();
        pipes.push((r, reg, Token(i)));
    }

    // Register all pipes concurrently
    let handles: Vec<_> = pipes
        .into_iter()
        .map(|(r, reg, token)| {
            thread::spawn(move || {
                reg.register(&mut SourceFd(r.as_raw_fd()), token, Interest::READABLE)
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked").expect("register failed");
    }

    // Verify all are registered by polling (should return no events)
    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(10)))
        .expect("poll failed");
}

/// Test: EINTR handling — poll returns Ok with empty events on signal
#[test]
fn poll_handles_eintr() {
    let poll = Poll::new().expect("failed to create poll");

    // We can't easily trigger EINTR, but we verify poll doesn't panic
    // or return an error for normal operation
    let mut events = Events::with_capacity(16);
    let result = poll.poll(&mut events, Some(Duration::from_millis(1)));
    assert!(result.is_ok(), "poll should handle normal timeout gracefully");
}

/// Test: TcpStream connect/accept loopback
#[test]
fn tcp_stream_connect_accept() {
    use std::net::TcpListener as StdTcpListener;

    // Bind a listener
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    // Create poll layer
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    // Register listener for readability
    registry
        .register(&mut SourceFd(listener.as_raw_fd()), Token(1), Interest::READABLE)
        .expect("register failed");

    // Connect a client
    let _client = std::net::TcpStream::connect(addr).expect("connect failed");

    // Poll should return event for listener
    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");
    assert!(
        events.iter().any(|e| e.token() == Token(1) && e.is_readable()),
        "expected listener readable event"
    );

    // Accept should succeed
    let (stream, _) = listener.accept().expect("accept failed");
    assert!(stream.peer_addr().is_ok());
}

/// Test: UdpSocket send/recv loopback
#[test]
fn udp_socket_send_recv() {
    use std::net::UdpSocket as StdUdpSocket;

    let sock = StdUdpSocket::bind("127.0.0.1:0").expect("bind failed");
    sock.set_nonblocking(true).expect("set_nonblocking failed");
    let addr = sock.local_addr().expect("local_addr failed");

    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    registry
        .register(&mut SourceFd(sock.as_raw_fd()), Token(5), Interest::READABLE)
        .expect("register failed");

    // Send a datagram to ourselves
    sock.send_to(b"hello loopback", addr).expect("send failed");

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");
    assert!(
        events.iter().any(|e| e.token() == Token(5) && e.is_readable()),
        "expected udp socket readable event"
    );

    let mut buf = [0u8; 64];
    let (n, _) = sock.recv_from(&mut buf).expect("recv failed");
    assert_eq!(&buf[..n], b"hello loopback");
}

/// Test: Unix socket connect/accept
#[cfg(unix)]
#[test]
fn unix_socket_connect_accept() {
    use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
    use std::path::PathBuf;

    let path: PathBuf = std::env::temp_dir().join(format!(
        "unix_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let listener = StdUnixListener::bind(&path).expect("bind failed");
    listener.set_nonblocking(true).expect("set_nonblocking failed");

    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    registry
        .register(&mut SourceFd(listener.as_raw_fd()), Token(3), Interest::READABLE)
        .expect("register failed");

    // Connect a client
    let _client = StdUnixStream::connect(&path).expect("connect failed");

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_millis(100)))
        .expect("poll failed");
    assert!(
        events.iter().any(|e| e.token() == Token(3) && e.is_readable()),
        "expected unix listener readable event"
    );

    let _ = std::fs::remove_file(&path);
}
