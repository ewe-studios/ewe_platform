//! Public-API tests for `foundation_iogate`.
//!
//! WHY: the I/O gate is the seam that turns a `ServerIo` mode into a concrete
//! netio `Connection`. These tests pin the enum's contract and prove that a real
//! socket accepted through the gate carries bytes in both `Std` and `Completion`
//! modes — the read/write parity Feature 48 Phase 2/3 promises.
//!
//! WHAT: `ServerIo` property tests, plus round-trip byte tests over a loopback
//! TCP connection accepted in each mode.
//!
//! HOW: bind an ephemeral listener, connect a client, accept through
//! `accept_connection`, and read what the client wrote. Reactor-touching tests
//! are `#[serial]` because the reactor is a process-global singleton.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use foundation_iogate::ServerIo;
use foundation_netio::netcap::Connection;

use serial_test::serial;
use tracing_test::traced_test;

/// Read `want` bytes from `conn`, retrying on `WouldBlock` up to `deadline`.
///
/// Both modes are nonblocking: `Std` is `read(2)` on a nonblocking socket,
/// `Completion` pops the io_uring inbox — both return `WouldBlock` when empty.
fn read_until(conn: &mut Connection, want: usize, deadline: Duration) -> Vec<u8> {
    let start = Instant::now();
    let mut got = Vec::with_capacity(want);
    let mut buf = [0u8; 4096];
    while got.len() < want {
        assert!(
            start.elapsed() < deadline,
            "timed out after {got:?} of {want} bytes",
            got = got.len(),
        );
        match conn.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => panic!("read failed: {e}"),
        }
    }
    got
}

/// Accept one connection from `listener` in `mode` and return the gated
/// `Connection`.
fn accept_one(listener: &TcpListener, mode: ServerIo) -> Connection {
    let (server_tcp, peer) = listener.accept().expect("accept");
    server_tcp.set_nonblocking(true).expect("nonblocking");
    foundation_iogate::accept_connection(server_tcp, peer, mode).expect("accept_connection")
}

#[test]
fn server_io_defaults_to_std() {
    assert_eq!(ServerIo::default(), ServerIo::Std);
    assert!(!ServerIo::Std.uses_reactor());
    assert!(!ServerIo::Std.is_completion());
}

#[test]
fn server_io_mode_flags() {
    // Every non-Std mode registers with the reactor.
    assert!(ServerIo::Readiness.uses_reactor());
    assert!(ServerIo::Completion.uses_reactor());
    assert!(ServerIo::Auto.uses_reactor());

    // Only Completion and Auto opt into the completion inbox.
    assert!(ServerIo::Completion.is_completion());
    assert!(ServerIo::Auto.is_completion());
    assert!(!ServerIo::Readiness.is_completion());
}

#[test]
fn server_io_display_is_stable() {
    assert_eq!(ServerIo::Std.to_string(), "std");
    assert_eq!(ServerIo::Readiness.to_string(), "readiness");
    assert_eq!(ServerIo::Completion.to_string(), "completion");
    assert_eq!(ServerIo::Auto.to_string(), "auto");
}

#[test]
#[traced_test]
fn std_mode_yields_plain_tcp_and_carries_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let mut client = TcpStream::connect(addr).expect("connect");
    let mut conn = accept_one(&listener, ServerIo::Std);

    // Std never touches the reactor.
    assert!(matches!(conn, Connection::Tcp(_)));

    client.write_all(b"hello std").expect("client write");
    client.flush().expect("flush");

    let got = read_until(&mut conn, b"hello std".len(), Duration::from_secs(5));
    assert_eq!(&got, b"hello std");
}

#[test]
#[serial]
#[traced_test]
fn completion_mode_carries_bytes() {
    // Completion mode demands io_uring. On a kernel without it, initialising the
    // reactor on `Uring` is a hard error — skip loudly rather than fail, since
    // this is an environment capability, not a defect.
    if foundation_iogate::init_reactor_for(ServerIo::Completion).is_err() {
        tracing::warn!("io_uring unavailable — skipping completion-mode byte test");
        return;
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    let mut client = TcpStream::connect(addr).expect("connect");
    let mut conn = accept_one(&listener, ServerIo::Completion);

    assert!(matches!(conn, Connection::Completion(_)));

    client.write_all(b"hello completion").expect("client write");
    client.flush().expect("flush");

    let got = read_until(&mut conn, b"hello completion".len(), Duration::from_secs(5));
    assert_eq!(&got, b"hello completion");
}

/// Spawn a one-shot server that accepts a connection, writes `msg`, and holds the
/// socket open briefly so the client can read before EOF. Returns its address.
fn one_shot_writer(msg: &'static [u8]) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let handle = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().expect("accept");
        s.write_all(msg).expect("server write");
        s.flush().ok();
        std::thread::sleep(Duration::from_millis(200));
    });
    (addr, handle)
}

#[test]
#[traced_test]
fn connect_completion_std_yields_plain_tcp_and_carries_bytes() {
    let (addr, server) = one_shot_writer(b"std upstream");

    let mut conn = foundation_iogate::connect_completion(addr, ServerIo::Std).expect("dial");
    // Std never touches the reactor.
    assert!(matches!(conn, Connection::Tcp(_)));

    let got = read_until(&mut conn, b"std upstream".len(), Duration::from_secs(5));
    assert_eq!(&got, b"std upstream");
    server.join().ok();
}

#[test]
#[serial]
#[traced_test]
fn connect_completion_dials_a_completion_backed_upstream() {
    // The client mirror of `completion_mode_carries_bytes`: the *outbound* dial is
    // registered with the reactor, so an upstream leg reads from the io_uring inbox
    // (Feature 50 Part A). Skip loudly without io_uring.
    if foundation_iogate::init_reactor_for(ServerIo::Completion).is_err() {
        tracing::warn!("io_uring unavailable — skipping completion-mode dial test");
        return;
    }

    let (addr, server) = one_shot_writer(b"from a completion upstream");

    let mut conn =
        foundation_iogate::connect_completion(addr, ServerIo::Completion).expect("dial");
    assert!(matches!(conn, Connection::Completion(_)));

    let got = read_until(&mut conn, b"from a completion upstream".len(), Duration::from_secs(5));
    assert_eq!(&got, b"from a completion upstream");
    server.join().ok();
}
