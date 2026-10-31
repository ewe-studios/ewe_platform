//! Feature 06 regression: rustls readiness follows the TLS state, not the
//! direction the application asked for.
//!
//! WHY: a plaintext *read* can stall on a TLS record rustls still has to
//! *write* (a key update, an alert). The old sync driver (`conn_io`) polled the
//! socket in the direction the application asked for, so a read on a session
//! that first had to write waited until its timeout. Feature 06 adds
//! `TlsInterest::corrected`, which maps "the application wants to read" to
//! "the TLS session wants to write" when it has pending output.
//!
//! WHAT: the test triggers a TLS key update mid-stream (the state that sets
//! `wants_write`) and asserts the readiness handle answers in the write
//! direction, so a driver that polls "readable" proceeds instead of stalling.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use foundation_netio::netcap::connection::{ConnReadiness, Connection};
use foundation_netio::netcap::ssl::rustls::RustlsStream;
use foundation_netio::netcap::ssl::SSLAcceptor;
use foundation_netio::native::readiness::IoInterest;
use foundation_testing::http::test_tls_connector;
use zeroize::Zeroizing;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/tcp_and_tls/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).expect("read TLS fixture")
}

/// Drive the client handshake to completion on a blocking socket.
fn finish_handshake(client: &RustlsStream<rustls::ClientConnection>) {
    loop {
        match client.drive_handshake() {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Blocking socket: `complete_io` blocks rather than returning
                // `WouldBlock`. Yield defensively; it must not spin.
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => panic!("handshake failed: {e}"),
        }
    }
}

/// A pending TLS write must override an application read.
///
/// The application asks for the read direction, but the TLS session still has
/// buffered output — here a key update it must send, the same state an alert or
/// a renegotiation leaves it in. `poll_fd` and `ready_now` must answer in the
/// write direction, or a driver that polls "readable" stalls.
#[test]
fn rustls_opposite_direction_does_not_stall() {
    let acceptor =
        SSLAcceptor::from_pem(fixture("cert.pem"), Zeroizing::new(fixture("key.pem")))
            .expect("acceptor");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");

    let server = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        let mut tls = acceptor.accept(Connection::Tcp(socket)).expect("tls accept");
        // `Read` completes the handshake, then blocks for the client's byte.
        let mut buf = [0u8; 16];
        let n = tls.read(&mut buf).expect("server read");
        assert!(n > 0, "client closed before its application byte");
        tls.write_all(b"ok").expect("server write");
        tls.flush().expect("server flush");
    });

    let plain = std::net::TcpStream::connect(addr).expect("connect");
    let (mut client, _addr) = test_tls_connector()
        .from_tcp_stream("localhost".to_string(), Connection::Tcp(plain))
        .expect("client tls stream");
    finish_handshake(&client);

    // Ask for a key update: rustls now has pending TLS output (`wants_write`).
    client.refresh_traffic_keys().expect("refresh traffic keys");

    let readiness: ConnReadiness = client.conn_readiness();

    // The corrected direction is WRITABLE, even though the caller asked to read.
    let (fd, corrected) = readiness
        .poll_fd(IoInterest::Readable)
        .expect("kernel-fd readiness");
    assert_eq!(
        corrected,
        IoInterest::Writable,
        "a pending TLS write must override the application's read direction"
    );
    // The driver therefore answers "ready now" at once, instead of stalling on
    // a socket that has nothing to read.
    assert!(
        readiness.ready_now(IoInterest::Readable).expect("probe"),
        "the read direction must report ready while the TLS session can write"
    );
    // The fd is the wrapped socket, so the driver polls the right place.
    assert_eq!(fd, client.as_raw_fd());

    // After the flush drains the pending output, the direction is the caller's.
    client.flush().expect("flush key update");
    let (_, back) = readiness
        .poll_fd(IoInterest::Readable)
        .expect("kernel-fd readiness");
    assert_eq!(
        back,
        IoInterest::Readable,
        "with no pending TLS output the read direction stands"
    );

    // Wake the server so its read returns and it can finish.
    client.write_all(b"x").expect("application byte");
    client.flush().expect("flush application byte");
    server.join().expect("server thread");
}
