//! An idle HTTP/2 connection task parks on its socket instead of polling.
//!
//! `H2ConnectionHandler` used to re-poll every 10 ms (1 ms during the
//! handshake) whenever no frame was buffered — the "47b" reactor-parking gap.
//! It now registers the socket with the shared reactor and returns
//! `TaskStatus::Depends`, resuming when the socket, a handler's response pipe,
//! the idle deadline or shutdown says so.

#![cfg(all(unix, not(target_family = "wasm")))]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::WaitGroup;
use foundation_http::ShutdownSignal;
use foundation_core::valtron::{initialize_pool, Ctx, TaskIterator, TaskStatus};
use foundation_http::native::server::H2ConnectionHandler;
use foundation_http::shared::app::HttpApp;
use foundation_netio::http2::channel::H2Channel;
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::http2::connection::{H2Request, CLIENT_PREFACE};
use foundation_netio::netcap::{ConnectionContext, RawStream};

#[test]
fn an_idle_h2_connection_parks_on_its_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    server.set_nonblocking(true).expect("non-blocking");

    // The client preface and an empty SETTINGS frame: the whole handshake.
    client.write_all(CLIENT_PREFACE).expect("preface");
    client
        .write_all(&[0, 0, 0, 0x4, 0, 0, 0, 0, 0])
        .expect("settings");

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).expect("raw stream"));
    let waits = WaitGroup::new();
    waits.add(1);
    let mut handler = H2ConnectionHandler::new(
        H2Conn::new_server(stream),
        Arc::new(HttpApp::new_h2_serve()),
        Arc::new(ConnectionContext::default()),
        Arc::new(ShutdownSignal::new()),
        waits.guard(),
    );

    let mut last = None;
    for _ in 0..50 {
        let status = handler.next_status(Ctx::new()).expect("the connection stays open");
        let parked = matches!(status, TaskStatus::Depends(_));
        assert!(
            !matches!(status, TaskStatus::Delayed(_)),
            "the handler polled on a timer instead of parking"
        );
        last = Some(parked);
        if parked {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(last, Some(true), "an idle connection must park (Depends)");
    drop(client);
}

// ── CPU regression tests (real pool + reactor) ──────────────────────────

/// Process CPU time (user + system) in nanoseconds, via `getrusage`.
fn process_cpu_ns() -> u128 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        let rc = libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        assert_eq!(rc, 0, "getrusage failed");
        let user = usage.ru_utime.tv_sec as u128 * 1_000_000_000
            + usage.ru_utime.tv_usec as u128 * 1_000;
        let system = usage.ru_stime.tv_sec as u128 * 1_000_000_000
            + usage.ru_stime.tv_usec as u128 * 1_000;
        user + system
    }
}

/// Write any pending channel output, then read whatever the server sent back.
/// The client socket is blocking with a short read timeout, so `read` returns
/// at once when the server answers and otherwise yields after the timeout.
fn pump_h2(stream: &mut TcpStream, chan: &mut H2Channel) {
    let out = chan.drain_output();
    if !out.is_empty() {
        stream.write_all(&out).expect("client write");
    }
    let mut buf = [0u8; 8192];
    match stream.read(&mut buf) {
        Ok(0) => return,
        Ok(n) => chan.feed_input(&buf[..n]),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut => {}
        Err(e) => panic!("client read: {e}"),
    }
}

/// Drive the `H2Channel` client handshake to completion over `stream`.
fn h2_client_handshake(stream: &mut TcpStream, chan: &mut H2Channel) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match chan.client_handshake_step() {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(std::time::Instant::now() < deadline, "handshake timed out");
                pump_h2(stream, chan);
            }
            Err(e) => panic!("client handshake: {e}"),
        }
    }
}

/// Send one `GET /` request and wait until its response headers arrive.
fn h2_client_get(stream: &mut TcpStream, chan: &mut H2Channel) {
    chan.send_request(&H2Request {
        method: Bytes::from_static(b"GET"),
        scheme: Bytes::from_static(b"http"),
        authority: Bytes::from_static(b"test"),
        path: Bytes::from_static(b"/"),
        headers: Vec::new(),
        body: None,
        end_stream: true,
    })
    .expect("send request");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        pump_h2(stream, chan);
        match chan.recv_response() {
            Ok(Some(_)) => return,
            Ok(None) => panic!("server sent GOAWAY"),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("recv response: {e}"),
        }
        assert!(std::time::Instant::now() < deadline, "response timed out");
    }
}

/// Start one H2 connection handler on the pool with a real loopback socket,
/// returning the client end once the handshake and one request are done.
#[cfg(feature = "multi")]
fn idle_h2_client() -> (TcpStream, H2Channel) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    server.set_nonblocking(true).expect("non-blocking");
    client
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("read timeout");

    let stream = SharedByteBufferStream::rwrite(RawStream::from_tcp(server).expect("raw stream"));
    let waits = WaitGroup::new();
    waits.add(1);
    let handler = H2ConnectionHandler::new(
        H2Conn::new_server(stream),
        Arc::new(HttpApp::new_h2_serve()),
        Arc::new(ConnectionContext::default()),
        Arc::new(ShutdownSignal::new()),
        waits.guard(),
    );
    foundation_core::valtron::send(handler).expect("spawn h2 handler");

    let mut chan = H2Channel::new(false);
    h2_client_handshake(&mut client, &mut chan);
    h2_client_get(&mut client, &mut chan);
    (client, chan)
}

/// An idle HTTP/2 connection uses no CPU: it parks on its socket, so a whole
/// second of silence costs under 50 ms of process CPU. The old 10 ms timer woke
/// it 100 times a second.
#[cfg(feature = "multi")]
#[test]
fn idle_h2_connection_uses_no_cpu() {
    let _guard = initialize_pool(100, Some(4));
    let (client, _chan) = idle_h2_client();

    // Let the connection settle into its parked state before measuring.
    std::thread::sleep(Duration::from_millis(100));
    let before = process_cpu_ns();
    std::thread::sleep(Duration::from_secs(1));
    let after = process_cpu_ns();
    let cpu_ms = (after - before) / 1_000_000;
    assert!(
        cpu_ms < 50,
        "idle h2 connection used {cpu_ms} ms of CPU in 1 s; it should park"
    );
    drop(client);
}

/// After a second idle, a request is served at once — the socket wake works,
/// so the parked handler runs again as soon as bytes arrive instead of waiting
/// for a poll cycle.
#[cfg(feature = "multi")]
#[test]
fn h2_request_after_idle_is_served_at_once() {
    let _guard = initialize_pool(101, Some(4));
    let (mut client, mut chan) = idle_h2_client();

    std::thread::sleep(Duration::from_secs(1));

    let start = std::time::Instant::now();
    h2_client_get(&mut client, &mut chan);
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(200),
        "request after idle took {elapsed:?}; the socket wake did not fire at once"
    );
    drop(client);
}
