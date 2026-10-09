//! QUIC backend acceptance tests (F33).
//!
//! WHY: F33's acceptance criterion is "QUIC connect/accept + bidi stream echo
//! **over the trait set**, parking on the reactor (no busy-poll)." Before this
//! suite the crate had a client-only driver, no trait set, no accept path, and no
//! QUIC test at all — so the criterion could not have been checked.
//!
//! WHAT: a real client and a real server, over loopback UDP, exchanging bytes on a
//! bidirectional stream through `QuicConnection` / `QuicBidiStream` — never
//! through `quinn_proto` directly. Plus the error mappings HTTP/3 depends on:
//! `Blocked` is `Pending` and not a failure; FIN is `Ok(None)`; a peer reset is
//! `Terminated`.
//!
//! HOW: both drivers are `TaskIterator`s, so a test can step them by hand with
//! `next_status()` rather than standing up a valtron pool. That keeps the test
//! deterministic and makes "did it park?" directly observable: an idle driver
//! returns `Depends`, never `Delayed`, once a readiness signal is injected.
//!
//! Certificates live in `tests/fixtures/quic_{cert,key}.pem`, generated with
//! openssl for `CN=localhost` / `IP:127.0.0.1`.

#![cfg(feature = "quic")]

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::{Buf, Bytes};
use foundation_core::valtron::{Stream, TaskIterator, TaskStatus};

use foundation_netio::quic::{
    client_config_trusting_pem, server_config_from_pem, QuicBidiStream, QuicConnection, QuicDriver,
    QuicRecvStream, QuicSendStream, QuinnConnection,
};

const CERT_PEM: &[u8] = include_bytes!("../fixtures/quic_cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../fixtures/quic_key.pem");
/// The client trusts the CA, not the leaf: a self-signed CA presented as an
/// end-entity certificate is rejected by rustls (`CaUsedAsEndEntity`).
const CA_PEM: &[u8] = include_bytes!("../fixtures/quic_ca.pem");

/// How long a handshake or echo may take before the test gives up.
const BUDGET: Duration = Duration::from_secs(5);

/// Step both drivers once. Returns `true` if either made progress.
fn pump(server: &mut QuicDriver, client: &mut QuicDriver) -> bool {
    let s = server.next_status();
    let c = client.next_status();
    matches!(s, Some(TaskStatus::Ready(_) | TaskStatus::Pending(_)))
        || matches!(c, Some(TaskStatus::Ready(_) | TaskStatus::Pending(_)))
}

/// Drive both sides until `done` returns `Some`, or the budget expires.
fn drive_until<T>(
    server: &mut QuicDriver,
    client: &mut QuicDriver,
    mut done: impl FnMut(&mut QuicDriver, &mut QuicDriver) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + BUDGET;
    while Instant::now() < deadline {
        if let Some(v) = done(server, client) {
            return Some(v);
        }
        pump(server, client);
    }
    None
}

/// Bring up a connected client/server pair on loopback.
fn connected_pair() -> (QuicDriver, QuicDriver, QuinnConnection, QuinnConnection) {
    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let client_cfg = client_config_trusting_pem(CA_PEM).expect("client config");

    let mut server =
        QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind server");
    let addr: SocketAddr = server.local_addr().expect("local addr");

    let (mut client, client_conn) =
        QuicDriver::connect(addr, client_cfg, "localhost").expect("connect");

    // The server's connection appears only once its handshake completes — that is
    // the accept point.
    let server_conn = drive_until(&mut server, &mut client, |s, _| s.take_accepted())
        .expect("server never accepted the connection");

    (server, client, server_conn, client_conn)
}

/// Read one chunk from a stream, driving the transports until it arrives.
fn read_chunk(
    server: &mut QuicDriver,
    client: &mut QuicDriver,
    stream: &mut impl QuicRecvStream,
) -> Option<Bytes> {
    drive_until(server, client, |s, c| match stream.read() {
        Stream::Next(Ok(Some(bytes))) => Some(Some(bytes)),
        // End of stream.
        Stream::Next(Ok(None)) => Some(None),
        Stream::Next(Err(e)) => panic!("stream read failed: {e}"),
        // Nothing buffered yet — that is `Pending`, not an error.
        _ => {
            let _ = (s, c);
            None
        }
    })
    .expect("read never completed within the budget")
}

/// Write every byte of `data`, driving the transports through flow control.
fn write_all(
    server: &mut QuicDriver,
    client: &mut QuicDriver,
    stream: &mut impl QuicSendStream,
    data: &[u8],
) {
    let mut buf = Bytes::copy_from_slice(data);
    drive_until(server, client, |_, _| {
        if !buf.has_remaining() {
            return Some(());
        }
        match stream.send(&mut buf) {
            Stream::Next(Ok(_)) => {}
            Stream::Next(Err(e)) => panic!("stream send failed: {e}"),
            // Flow-control window full. This is the back-pressure signal, and the
            // reason h3's `poll_ready` is unnecessary here.
            Stream::Pending(()) => {}
            _ => {}
        }
        None
    })
    .expect("write never drained within the budget");
}

#[test]
fn client_connects_and_server_accepts() {
    let (mut server, mut client, mut server_conn, mut client_conn) = connected_pair();

    // `connected_pair` already proves the server accepted. What is worth asserting
    // beyond that is that the connection is *usable*: both ends can open a stream,
    // which only succeeds once the handshake has completed.
    let opened = drive_until(&mut server, &mut client, |_, _| {
        match client_conn.open_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("open_bidi after handshake failed: {e}"),
            _ => None,
        }
    });
    assert!(
        opened.is_some(),
        "an established connection must be able to open a stream"
    );

    // And the server side has not gone anywhere.
    assert!(
        matches!(
            server_conn.accept_bidi(),
            Stream::Pending(()) | Stream::Next(Ok(_))
        ),
        "the accepted connection must still be live"
    );
}

#[test]
fn bidi_stream_echo_over_the_trait_set() {
    let (mut server, mut client, mut server_conn, mut client_conn) = connected_pair();

    // Client opens a bidirectional stream. `Pending` here means the peer's
    // MAX_STREAMS limit, not an error — keep driving.
    let mut client_stream = drive_until(&mut server, &mut client, |_, _| {
        match client_conn.open_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("open_bidi failed: {e}"),
            _ => None,
        }
    })
    .expect("client never opened a bidi stream");

    assert!(
        client_stream.id().is_client_initiated() && client_stream.id().is_bidirectional(),
        "a client-opened bidi stream must have the RFC 9000 id bits: {}",
        client_stream.id()
    );

    write_all(
        &mut server,
        &mut client,
        &mut client_stream,
        b"ping over quic",
    );

    // Server accepts it.
    let mut server_stream = drive_until(&mut server, &mut client, |_, _| {
        match server_conn.accept_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("accept_bidi failed: {e}"),
            _ => None,
        }
    })
    .expect("server never accepted the bidi stream");

    assert_eq!(
        server_stream.id(),
        client_stream.id(),
        "both ends must agree on the stream id"
    );

    let got = read_chunk(&mut server, &mut client, &mut server_stream)
        .expect("server read end-of-stream instead of data");
    assert_eq!(&got[..], b"ping over quic");

    // Echo it back on the same stream, then finish.
    write_all(&mut server, &mut client, &mut server_stream, &got);
    drive_until(&mut server, &mut client, |_, _| {
        match server_stream.finish() {
            Stream::Next(Ok(())) => Some(()),
            Stream::Next(Err(e)) => panic!("finish failed: {e}"),
            _ => None,
        }
    })
    .expect("server finish never completed");

    let echoed = read_chunk(&mut server, &mut client, &mut client_stream)
        .expect("client read end-of-stream instead of the echo");
    assert_eq!(&echoed[..], b"ping over quic", "the echo must round-trip");

    // After FIN, the read half reports end-of-stream, repeatedly.
    let after = read_chunk(&mut server, &mut client, &mut client_stream);
    assert!(
        after.is_none(),
        "a finished stream must report end-of-stream, got {after:?}"
    );
    assert!(
        matches!(client_stream.read(), Stream::Next(Ok(None))),
        "end-of-stream must latch"
    );
}

#[test]
fn split_yields_both_halves_of_a_bidi_stream() {
    let (mut server, mut client, mut server_conn, mut client_conn) = connected_pair();

    let client_stream = drive_until(&mut server, &mut client, |_, _| {
        match client_conn.open_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("open_bidi failed: {e}"),
            _ => None,
        }
    })
    .expect("open_bidi");

    let id = client_stream.id();
    let (mut send, recv) = client_stream.split();
    assert_eq!(send.id(), id, "the write half keeps the stream id");
    assert_eq!(recv.id(), id, "the read half keeps the stream id");

    write_all(&mut server, &mut client, &mut send, b"split");

    let mut server_stream = drive_until(&mut server, &mut client, |_, _| {
        match server_conn.accept_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("accept_bidi failed: {e}"),
            _ => None,
        }
    })
    .expect("accept_bidi");

    let got = read_chunk(&mut server, &mut client, &mut server_stream).expect("data");
    assert_eq!(&got[..], b"split");
}

#[test]
fn unidirectional_stream_open_and_accept() {
    let (mut server, mut client, mut server_conn, mut client_conn) = connected_pair();

    // HTTP/3 control and QPACK streams are unidirectional; this is the path they
    // take.
    let mut send = drive_until(&mut server, &mut client, |_, _| {
        match client_conn.open_send() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("open_send failed: {e}"),
            _ => None,
        }
    })
    .expect("open_send");

    assert!(
        send.id().is_unidirectional() && send.id().is_client_initiated(),
        "id bits must say client-initiated unidirectional: {}",
        send.id()
    );

    write_all(&mut server, &mut client, &mut send, b"control");

    let mut recv = drive_until(&mut server, &mut client, |_, _| {
        match server_conn.accept_recv() {
            Stream::Next(Ok(s)) => Some(s),
            Stream::Next(Err(e)) => panic!("accept_recv failed: {e}"),
            _ => None,
        }
    })
    .expect("accept_recv");

    assert_eq!(recv.id(), send.id());
    let got = read_chunk(&mut server, &mut client, &mut recv).expect("data");
    assert_eq!(&got[..], b"control");
}

#[test]
fn peer_reset_surfaces_as_terminated() {
    let (mut server, mut client, mut server_conn, mut client_conn) = connected_pair();

    let mut client_stream = drive_until(&mut server, &mut client, |_, _| {
        match client_conn.open_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            _ => None,
        }
    })
    .expect("open_bidi");

    write_all(&mut server, &mut client, &mut client_stream, b"x");

    let mut server_stream = drive_until(&mut server, &mut client, |_, _| {
        match server_conn.accept_bidi() {
            Stream::Next(Ok(s)) => Some(s),
            _ => None,
        }
    })
    .expect("accept_bidi");

    let _ = read_chunk(&mut server, &mut client, &mut server_stream);

    // Server resets its write half with an application code. The client's read
    // half must report `Terminated` carrying that code — HTTP/3 maps this onto
    // its own error space, so the code cannot be swallowed.
    server_stream.reset(42);

    let err = drive_until(&mut server, &mut client, |_, _| {
        match client_stream.read() {
            Stream::Next(Err(e)) => Some(e),
            Stream::Next(Ok(None)) => panic!("a reset stream must not look like a clean FIN"),
            _ => None,
        }
    })
    .expect("client never observed the reset");

    match err {
        foundation_netio::quic::QuicStreamError::Terminated { code } => {
            assert_eq!(code, 42, "the application error code must survive");
        }
        other => panic!("expected Terminated, got {other}"),
    }
}

#[test]
fn idle_driver_parks_on_injected_readiness_instead_of_spinning() {
    use foundation_core::valtron::EventReadiness;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// A readiness signal we control, standing in for `RegisteredFd`.
    struct Never(AtomicBool);
    impl EventReadiness for Never {
        fn is_ready(&self, _dur: Option<Duration>) -> bool {
            self.0.load(Ordering::Acquire)
        }
    }

    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let readiness = Arc::new(Never(AtomicBool::new(false)));

    let mut server = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg)
        .expect("bind server")
        .with_fd_readiness(readiness);

    // No datagrams, no connections: the driver has nothing to do. It must park on
    // the reactor, not burn a worker with `Delayed`.
    let status = server.next_status().expect("server driver keeps running");
    assert!(
        matches!(status, TaskStatus::Depends(_)),
        "an idle driver with an injected readiness signal must park via Depends, \
         not busy-poll; got a different TaskStatus"
    );
}

#[test]
fn idle_driver_without_readiness_falls_back_to_delayed() {
    let server_cfg = server_config_from_pem(CERT_PEM, KEY_PEM).expect("server config");
    let mut server = QuicDriver::server("127.0.0.1:0".parse().unwrap(), server_cfg).expect("bind");

    let status = server.next_status().expect("server driver keeps running");
    assert!(
        matches!(status, TaskStatus::Delayed(_)),
        "without a readiness signal the driver must yield with Delayed rather than \
         spin the executor"
    );
}
