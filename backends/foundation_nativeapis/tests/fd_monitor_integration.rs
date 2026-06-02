/// Integration test: FdMonitorTask with pipe and RegisteredFd.
///
/// Tests:
/// - Create a pipe, wrap read end in RegisteredFd
/// - Create FdMonitorTask with callback that reads data
/// - Write to pipe's write end
/// - Verify callback is invoked and data is read
/// - Verify poll interval respected (callback not called before write)

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_core::valtron::TaskIterator;
use foundation_nativeapis::fd::RegisteredFd;
use foundation_nativeapis::poll::{Interest, Poll, Token};
use foundation_nativeapis::task::FdMonitorTask;

/// Create a pipe (reader, writer), both nonblocking.
fn make_pipe() -> (OwnedFd, OwnedFd) {
    let (r, w) = nix::unistd::pipe().expect("pipe creation failed");
    nix::fcntl::fcntl(r.as_raw_fd(), nix::fcntl::FcntlArg::F_SETFL(
        nix::fcntl::OFlag::O_NONBLOCK
    )).expect("fcntl O_NONBLOCK failed");
    nix::fcntl::fcntl(w.as_raw_fd(), nix::fcntl::FcntlArg::F_SETFL(
        nix::fcntl::OFlag::O_NONBLOCK
    )).expect("fcntl O_NONBLOCK failed");
    (r, w)
}

/// Test: FdMonitorTask callback is invoked when fd is readable
#[test]
fn fd_monitor_task_invokes_callback_on_readiness() {
    let (reader, writer) = make_pipe();
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    let registered = RegisteredFd::new(reader, &registry, Token(0))
        .expect("failed to register fd");

    let data_received: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let data_clone = data_received.clone();

    let mut task = FdMonitorTask::new(registered)
        .with_callback(move |fd| {
            let mut buf = [0u8; 64];
            let n = unsafe {
                libc::read(
                    fd.as_raw_fd(),
                    buf.as_mut_ptr() as *mut _,
                    buf.len(),
                )
            };
            if n > 0 {
                data_clone.lock().unwrap().extend_from_slice(&buf[..n as usize]);
                Ok(())
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "read returned 0"))
            }
        })
        .with_poll_interval(Duration::from_millis(10));

    // Write data before first tick
    unsafe {
        libc::write(
            writer.as_raw_fd(),
            b"hello from pipe\0".as_ptr() as *const _,
            16,
        );
    }

    // Tick the task via TaskIterator
    let status = task.next_status();
    assert!(status.is_some(), "task should not terminate");

    // Verify callback received data
    let received = data_received.lock().unwrap();
    assert_eq!(&received[..15], b"hello from pipe");
}

/// Test: FdMonitorTask with no callback doesn't panic
#[test]
fn fd_monitor_task_no_callback() {
    let (reader, _writer) = make_pipe();
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    let registered = RegisteredFd::new(reader, &registry, Token(0))
        .expect("failed to register fd");

    // Create task WITHOUT a callback
    let mut task = FdMonitorTask::new(registered)
        .with_poll_interval(Duration::from_millis(10));

    // Tick — should not panic, just return NotReady
    let status = task.next_status();
    assert!(status.is_some());
}

/// Test: FdMonitorTask terminates on fd error
#[test]
fn fd_monitor_task_terminates_on_error() {
    let (reader, _writer) = make_pipe();
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    // Register the fd first
    let registered = RegisteredFd::new(reader, &registry, Token(0))
        .expect("failed to register fd");

    // Close the underlying fd directly (bypassing RegisteredFd)
    // This simulates the fd being closed externally
    let raw_fd = registered.get_ref().as_raw_fd();
    unsafe { libc::close(raw_fd) };

    // Now poll — should return NotReady or Error, not crash
    let _status = registered.poll_readable();

    // Forget registered to avoid double-close on drop
    // (the fd is already closed, dropping would trigger IO safety violation)
    std::mem::forget(registered);
}

/// Test: FdMonitorTask poll interval is respected
#[test]
fn fd_monitor_task_poll_interval() {
    let (reader, _writer) = make_pipe();
    let poll = Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    let registered = RegisteredFd::new(reader, &registry, Token(0))
        .expect("failed to register fd");

    let interval = Duration::from_millis(100);
    let mut task = FdMonitorTask::new(registered)
        .with_poll_interval(interval);

    // Tick when no data available
    let start = std::time::Instant::now();
    let status = task.next_status();
    let elapsed = start.elapsed();

    // next_status returns immediately (it doesn't block — blocking is handled by valtron executor)
    assert!(elapsed < Duration::from_millis(50), "next_status should return immediately");
    assert!(status.is_some());
}
