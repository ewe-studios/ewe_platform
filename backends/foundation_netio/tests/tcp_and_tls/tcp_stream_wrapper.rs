//! `TcpStreamWrapper` behaviour (moved out of `native/connection.rs`).
//!
//! WHY: CLAUDE.md rule 7 — tests live in `tests/`, never inline in a source file.

use std::io::Write;
use std::net::{Shutdown, TcpListener};
use std::time::Duration;

use foundation_netio::netcap::connection::TcpStreamWrapper;

/// The common socket options round-trip and the write/peek/shutdown surface work.
#[test]
fn wrapper_standard() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let mut wrapper = TcpStreamWrapper::connect(addr).unwrap();

    wrapper.write_all(b"test").unwrap();
    wrapper.flush().unwrap();

    wrapper.set_nodelay(true).unwrap();
    assert!(wrapper.nodelay().unwrap());

    wrapper.set_ttl(100).unwrap();
    assert_eq!(wrapper.ttl().unwrap(), 100);

    let timeout = Some(Duration::from_secs(1));
    wrapper.set_read_timeout(timeout).unwrap();
    wrapper.set_write_timeout(timeout).unwrap();
    assert_eq!(wrapper.read_timeout().unwrap(), timeout);
    assert_eq!(wrapper.write_timeout().unwrap(), timeout);

    let mut buf = [0u8; 128];
    let result = wrapper.peek(&mut buf);
    assert!(result.is_ok() || result.unwrap_err().kind() == std::io::ErrorKind::WouldBlock);

    assert!(wrapper.take_error().unwrap().is_none());

    wrapper.shutdown(Shutdown::Both).unwrap();
}

/// A connect to a port with no listener fails.
#[test]
fn connect_timeout() {
    let addr = "127.0.0.1:54321".parse().expect("generate addrs");
    let result = TcpStreamWrapper::connect_timeout(&addr, Duration::from_millis(100));
    assert!(result.is_err());
}

/// The `nightly`-gated socket options round-trip.
#[cfg(feature = "nightly")]
#[test]
fn wrapper_nightly() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let mut wrapper = TcpStreamWrapper::connect(addr).unwrap();

    wrapper.set_linger(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(wrapper.linger().unwrap(), Some(Duration::from_secs(1)));

    let _local_addr = wrapper.local_addr().expect("as local address");
    let _peer_addr = wrapper.peer_addr().expect("expect peer addr");

    wrapper.set_reuse_address(true).unwrap();
    assert!(wrapper.reuse_address().unwrap());
}
