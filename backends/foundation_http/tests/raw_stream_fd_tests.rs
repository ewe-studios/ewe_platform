//! Integration test for the RawStream → RegisteredFd seam (Decision 12 §12 of
//! spec 41-connectrpc, feature F09).
//!
//! WHY: The native parking model registers a connection's socket fd with the
//! `foundation_nativeapis` reactor to obtain a `RegisteredFd: EventReadiness` to
//! park on. That requires the raw fd to be reachable from **above** `netio`.
//! This test exercises exactly that path from a crate that sits above `netio`:
//! wrap a live socket in a `netcap::RawStream`, read its fd via `AsRawFd`, and
//! build a `RegisteredFd` from it.
#![cfg(unix)]

use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;

use foundation_nativeapis::native::fd::RegisteredFd;
use foundation_nativeapis::{Poll, Token};
use foundation_netio::netcap::RawStream;

/// A live `RawStream` yields its fd and a `RegisteredFd` builds from it.
#[test]
fn raw_stream_fd_registers_with_reactor() {
    // Establish a live TCP connection and take the accepted server side.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let _client = TcpStream::connect(addr).expect("connect");
    let (server, _peer) = listener.accept().expect("accept");
    // RegisteredFd requires the fd to be nonblocking for correct operation.
    server.set_nonblocking(true).expect("set_nonblocking");

    let raw = RawStream::from_tcp(server).expect("wrap TcpStream in RawStream");

    // The RawStream exposes the underlying socket fd through the buffered wrapper.
    let stream_fd = raw.as_raw_fd();
    assert!(stream_fd >= 0, "RawStream must expose a valid fd");

    // Build a RegisteredFd from the live RawStream via the native reactor.
    let poll = Poll::new().expect("create poll");
    let registry = poll.registry();
    let registered =
        RegisteredFd::new(raw, &registry, Token(0)).expect("register RawStream fd with reactor");

    // The registered fd is the same fd the RawStream reported.
    assert_eq!(
        registered.as_raw_fd(),
        stream_fd,
        "RegisteredFd should wrap the RawStream's socket fd"
    );
}
