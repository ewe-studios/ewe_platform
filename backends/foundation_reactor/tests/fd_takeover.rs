//! A second registration of one descriptor must take it over.
//!
//! WHY: several layers register one connection descriptor: the HTTP server's
//! idle reader and writer, and the RPC connection driver. The selector keeps
//! one entry per descriptor, and the latest registration owns it. The Linux
//! selector (`epoll`) rejects a second `ADD` of one descriptor with
//! `EEXIST`, which closed every RPC connection on Linux before the reactor
//! removed the earlier entry first. The kqueue selector replaced the token
//! silently, so macOS never showed the defect.
//!
//! WHAT: register one socket descriptor twice, write to its peer, and assert
//! that the second registration reports the readability.
//!
//! HOW: real local sockets through the process-wide reactor. This is its own
//! test binary because the reactor is a process singleton.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use foundation_reactor::FdReadiness;

/// The bound for one wait. Long enough for a loaded runner, short enough to
/// fail a lost wake quickly.
const WAIT_BOUND: Duration = Duration::from_secs(2);

#[test]
fn a_second_registration_of_one_descriptor_takes_it_over() {
    let (mut peer, mut socket) = UnixStream::pair().expect("socketpair");
    socket
        .set_nonblocking(true)
        .expect("nonblocking socket end");
    let fd = socket.as_raw_fd();

    let first = FdReadiness::register(fd).expect("the first registration");
    // The Linux selector returned `EEXIST` here before the fix.
    let second = FdReadiness::register(fd).expect("the second registration takes the fd over");

    peer.write_all(b"x").expect("write to the peer");
    peer.flush().expect("flush the peer");

    let deadline = Instant::now() + WAIT_BOUND;
    while Instant::now() < deadline {
        let generation = second.events_generation();
        if second.ready().is_readable() {
            break;
        }
        let _ = second.wait_for_events(generation, Duration::from_millis(100));
    }
    assert!(
        second.ready().is_readable(),
        "the second registration reports the readiness of fd {fd}"
    );

    let mut buf = [0u8; 1];
    socket.read_exact(&mut buf).expect("the byte is readable");
    drop(first);
}
