/// Example: File descriptor readiness tracking.
///
/// Demonstrates how to use RegisteredFd with the poll selector to monitor
/// an arbitrary file descriptor for readability. This is the building block
/// that FdMonitorTask uses under the hood.

use std::io;
use std::os::fd::AsRawFd;
use std::time::Duration;

use foundation_nativeapis::native::fd::{PollResult, RegisteredFd};
use foundation_nativeapis::{FdState, Interest, Poll, Token};

fn main() -> io::Result<()> {
    let (reader, writer) = make_pipe()?;

    let poll = Poll::new()?;
    let registry = poll.registry();

    let registered = RegisteredFd::with_interest(
        reader,
        &registry,
        Token(0),
        Interest::READABLE | Interest::WRITABLE,
    )?;

    println!("Registered pipe reader with poll selector");
    println!("Writing data to pipe...");

    write_to_fd(writer.as_raw_fd(), b"hello from pipe!")?;

    let fd_state = poll_for_readiness(&poll)?;
    println!("Fd became ready: {:?}", fd_state);

    match registered.poll_readable() {
        PollResult::Ready(mut guard) => {
            let mut buf = [0u8; 64];
            let result = guard.try_io_read(|fd| {
                let n = unsafe {
                    libc::read(
                        fd.as_raw_fd(),
                        buf.as_mut_ptr() as *mut _,
                        buf.len(),
                    )
                };
                if n < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok((&buf[..n as usize], n as usize))
                }
            });
            match result {
                Ok(Ok((data, n))) => {
                    println!("Read {} bytes: {:?}", n, String::from_utf8_lossy(data));
                }
                Ok(Err(e)) => println!("I/O error: {}", e),
                Err(e) => println!("try_io error: {}", e),
            }
        }
        PollResult::Error(e) => println!("Fd error: {}", e),
        PollResult::NotReady => println!("Fd not ready (unexpected)"),
    }

    drop(writer);
    std::thread::sleep(Duration::from_millis(10));

    match registered.poll_readable() {
        PollResult::Error(e) => println!("After writer closed: {} (correctly detected EOF)", e),
        other => println!("After writer closed: {:?} (unexpected)", other),
    }

    println!("\nDone — the fd was properly deregistered on drop");
    Ok(())
}

fn make_pipe() -> io::Result<(std::os::unix::io::OwnedFd, std::os::unix::io::OwnedFd)> {
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use nix::unistd::pipe;

    let (r, w) = pipe()?;
    fcntl(r.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    fcntl(w.as_raw_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    Ok((r, w))
}

fn write_to_fd(fd: std::os::unix::io::RawFd, data: &[u8]) -> io::Result<()> {
    let n = unsafe {
        libc::write(fd, data.as_ptr() as *const libc::c_void, data.len())
    };
    if n < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn poll_for_readiness(poll: &Poll) -> io::Result<FdState> {
    use foundation_nativeapis::Events;

    let mut events = Events::with_capacity(16);
    poll.poll(&mut events, Some(Duration::from_secs(5)))?;

    for event in events.iter() {
        if event.is_readable() && event.is_writable() {
            return Ok(FdState::Both);
        } else if event.is_readable() {
            return Ok(FdState::Readable);
        } else if event.is_writable() {
            return Ok(FdState::Writable);
        }
    }

    Err(io::Error::new(io::ErrorKind::TimedOut, "no readiness events within timeout"))
}
