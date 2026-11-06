//! Feature 125 — the H2 client pump parks on reactor readiness, not on a timer.
//!
//! WHY: before this feature every wait in `H2Pump::next_status` returned
//! `TaskStatus::Delayed(10 ms)`. The pump therefore ran 100 times a second for
//! the whole life of a call, even when the socket had no bytes and the caller
//! had queued no body chunk. Feature 121 measured the cost: a worker that hosts
//! a long task delays every `Delayed` task it hosts, and an idle H2 call could
//! stall for 1.5 s.
//!
//! WHAT: three tests drive a real `H2Transport` against a real socket on
//! `127.0.0.1:0` and count pump turns with `h2_pump_poll_count()`:
//! - `an_idle_h2_call_does_not_busy_poll`: the peer accepts and sends nothing.
//!   The 200 ms handshake timeout ends the call, and the pump may run at most
//!   5 times across that window. The old code runs about 20 times.
//! - `a_slow_response_body_arrives_without_a_poll`: the peer sends the response
//!   head at once and the body 300 ms later. Both arrive, and the pump turns
//!   stay small, which proves the socket edge wakes the pump.
//! - `a_response_body_pop_does_not_wake_the_pump` (feature 126): three body
//!   chunks arrive one interval apart while the consumer pops each one after
//!   the pump has parked. Only the socket edges cost a turn; a pop does not
//!   wake the parked pump.
//!
//! HOW: a raw `H2Channel` server on a plain `TcpStream` gives exact control of
//! the frame timing. `#[valtron_test]` provides the pool the pump runs on, and
//! `initialize_pool` serializes pool users, so the process-wide poll counter
//! cannot be changed by the other test at the same time.

// The park needs the shared reactor, which exists on unix only. Off unix the
// pump keeps the fixed `POLL_DELAY` fallback, so these tests do not apply.
#![cfg(unix)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;

use foundation_core::url::Uri;
use foundation_core::valtron::valtron_test;
use foundation_netio::http2::channel::H2Channel;
use foundation_netio::http2::frame::Kind;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeaders, SimpleMethod, SimpleUrl, Status,
};

use foundation_connectrpc::native::transport::h2_pump_poll_count;
use foundation_connectrpc::{H2Transport, Transport, TransportError};

/// The most pump turns an idle 200 ms call may take. The old `Delayed(10 ms)`
/// loop takes about 20.
const MAX_IDLE_POLLS: u64 = 5;

/// The most pump turns a call with a 300 ms delayed body may take. The old
/// `Delayed(10 ms)` loop takes about 30.
const MAX_DELAYED_BODY_POLLS: u64 = 10;

/// The most pump turns the body phase may take. Three body chunks arrive, so
/// three socket edges must wake the pump. Two more turns cover thread
/// scheduling and the final `Done` turn. A park on the shared vacancy signal
/// adds one turn per consumer pop, which is three more.
const MAX_BODY_PHASE_TURNS: u64 = 5;

/// The peer sends one body chunk every `BODY_CHUNK_INTERVAL`.
const BODY_CHUNK_INTERVAL: Duration = Duration::from_millis(200);

/// The consumer waits this long before each pop. It is longer than
/// `BODY_CHUNK_INTERVAL` (so the chunk is buffered) but leaves at least 160 ms
/// before the next chunk arrives. The pump has therefore parked before the pop,
/// which is the condition that tells the two signals apart. A pop that races
/// the pump's park wakes nobody, even on the old signal.
const BODY_POP_DELAY: Duration = Duration::from_millis(220);

fn request_descriptor(url: &str) -> RequestDescriptor {
    RequestDescriptor {
        proto: Proto::HTTP20,
        request_url: SimpleUrl::url_only(url.to_string()),
        request_uri: Uri::parse(url).expect("valid uri"),
        headers: SimpleHeaders::new(),
        method: SimpleMethod::POST,
    }
}

/// Write every queued frame to the socket.
fn flush_frames(channel: &mut H2Channel, socket: &mut TcpStream) {
    let out = channel.drain_output();
    if !out.is_empty() {
        socket.write_all(&out).expect("server write");
        socket.flush().expect("server flush");
    }
}

/// Read the bytes the socket has ready into the channel. The read timeout
/// makes this loop yield instead of blocking when the client is not sending.
fn feed_available(channel: &mut H2Channel, socket: &mut TcpStream) {
    let mut buf = [0u8; 8192];
    match socket.read(&mut buf) {
        Ok(0) => panic!("client closed the connection"),
        Ok(n) => channel.feed_input(&buf[..n]),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) => {}
        Err(e) => panic!("server read failed: {e}"),
    }
}

/// Run the server side of the h2 handshake to completion.
fn server_handshake(channel: &mut H2Channel, socket: &mut TcpStream) {
    loop {
        match channel.server_handshake_step() {
            Ok(()) => {
                flush_frames(channel, socket);
                return;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                flush_frames(channel, socket);
                feed_available(channel, socket);
            }
            Err(e) => panic!("server handshake failed: {e}"),
        }
    }
}

/// Read frames until the request HEADERS arrives, and return its stream id.
fn read_request_stream_id(channel: &mut H2Channel, socket: &mut TcpStream) -> u32 {
    loop {
        match channel.read_frame() {
            Ok((head, _payload)) => {
                flush_frames(channel, socket);
                if head.kind == Kind::Headers {
                    return head.stream_id;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                flush_frames(channel, socket);
                feed_available(channel, socket);
            }
            Err(e) => panic!("server frame read failed: {e}"),
        }
    }
}

/// A peer that accepts the connection and sends nothing. The handshake timeout
/// must end the call, and the pump must park while it waits.
#[valtron_test(timeout = 5_000)]
async fn an_idle_h2_call_does_not_busy_poll() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");

    // Accept the dial and hold the socket open without writing a byte.
    let peer = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
            }
        }
    });

    let transport = H2Transport::new().with_handshake_timeout(Duration::from_millis(200));
    let url = format!("http://{addr}/t.Svc/Unary");

    let before = h2_pump_poll_count();
    let mut call = transport
        .open(request_descriptor(&url))
        .expect("open the call");
    let head = call.head.next().await.expect("the call publishes a head");
    let after = h2_pump_poll_count();

    assert!(
        matches!(&head, Err(TransportError::Timeout)),
        "a silent peer must fail the handshake with Timeout, got {head:?}"
    );
    let polls = after - before;
    assert!(
        polls <= MAX_IDLE_POLLS,
        "an idle call must park instead of polling every 10 ms: \
         {polls} pump turns in a 200 ms window (max {MAX_IDLE_POLLS})"
    );

    drop(call);
    peer.join().expect("peer thread");
}

/// Each response-body pop must not wake the parked pump. The pump parks on the
/// response pipe's close-only readiness, so only the socket edges (the three
/// body chunks) cost a turn. A park on the shared vacancy signal adds one turn
/// per pop.
#[valtron_test(timeout = 5_000)]
async fn a_response_body_pop_does_not_wake_the_pump() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let body = Bytes::from_static(b"body-chunk");
    let peer_body = body.clone();

    let peer = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");

        let mut channel = H2Channel::new(true);
        server_handshake(&mut channel, &mut socket);
        let stream_id = read_request_stream_id(&mut channel, &mut socket);

        // The head goes out at once. Three chunks follow, one per interval.
        // None ends the stream: the pump must stay parked and alive between
        // them, so each consumer pop finds it parked.
        let headers = vec![(
            Bytes::from_static(b"content-type"),
            Bytes::from_static(b"application/octet-stream"),
        )];
        channel
            .send_headers_response(stream_id, 200, &headers, false)
            .expect("response headers");
        flush_frames(&mut channel, &mut socket);

        for _ in 0..3 {
            thread::sleep(BODY_CHUNK_INTERVAL);
            channel
                .send_data_frame(stream_id, &peer_body, false)
                .expect("response body");
            flush_frames(&mut channel, &mut socket);
        }

        // Keep the connection open until the client has read the body.
        thread::sleep(Duration::from_millis(500));
    });

    let transport = H2Transport::new();
    let url = format!("http://{addr}/t.Svc/Chunks");

    let mut call = transport
        .open(request_descriptor(&url))
        .expect("open the call");
    call.send_body.close();

    let (status, _headers) = call
        .head
        .next()
        .await
        .expect("the call publishes a head")
        .expect("the head is Ok");
    assert_eq!(status, Status::OK);

    // Count only the body phase: the head and its own wake are behind us.
    let before = h2_pump_poll_count();
    for expected in 0..3 {
        // Wait until the chunk is buffered and the pump has parked, then pop.
        foundation_core::valtron::sleep_async(BODY_POP_DELAY).await;
        let chunk = call
            .recv_body
            .next()
            .await
            .unwrap_or_else(|| panic!("chunk {} never arrived", expected + 1))
            .expect("body chunk");
        assert_eq!(chunk, body, "chunk {} carries the sent bytes", expected + 1);
    }
    // Give a wake that a pop triggers on the old signal time to reach the
    // pump's executor before the count is read.
    foundation_core::valtron::sleep_async(Duration::from_millis(50)).await;

    let turns = h2_pump_poll_count() - before;
    eprintln!("body phase: {turns} pump turns for three socket edges");
    assert!(
        turns <= MAX_BODY_PHASE_TURNS,
        "a body pop must not wake the pump: {turns} turns for three socket edges \
         (max {MAX_BODY_PHASE_TURNS})"
    );

    peer.join().expect("peer thread");
}

/// The response head arrives at once and the body 300 ms later. The pump must
/// wake on the body's socket edge rather than on a re-poll timer.
#[valtron_test(timeout = 5_000)]
async fn a_slow_response_body_arrives_without_a_poll() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let body = Bytes::from_static(b"body-after-300ms");
    let peer_body = body.clone();

    let peer = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");

        let mut channel = H2Channel::new(true);
        server_handshake(&mut channel, &mut socket);
        let stream_id = read_request_stream_id(&mut channel, &mut socket);

        // The response head goes out at once; the body follows 300 ms later.
        let headers = vec![(
            Bytes::from_static(b"content-type"),
            Bytes::from_static(b"application/octet-stream"),
        )];
        channel
            .send_headers_response(stream_id, 200, &headers, false)
            .expect("response headers");
        flush_frames(&mut channel, &mut socket);

        thread::sleep(Duration::from_millis(300));
        channel
            .send_data_frame(stream_id, &peer_body, true)
            .expect("response body");
        flush_frames(&mut channel, &mut socket);

        // Keep the connection open until the client has read the body.
        thread::sleep(Duration::from_millis(200));
    });

    let transport = H2Transport::new();
    let url = format!("http://{addr}/t.Svc/Slow");

    let before = h2_pump_poll_count();
    let mut call = transport
        .open(request_descriptor(&url))
        .expect("open the call");
    call.send_body.close();

    let (status, _headers) = call
        .head
        .next()
        .await
        .expect("the call publishes a head")
        .expect("the head is Ok");
    assert_eq!(status, Status::OK);

    let mut got = Vec::new();
    while let Some(chunk) = call.recv_body.next().await {
        got.extend_from_slice(&chunk.expect("body chunk"));
    }
    let after = h2_pump_poll_count();

    assert_eq!(got, body.to_vec(), "the delayed body must arrive whole");
    let polls = after - before;
    assert!(
        polls <= MAX_DELAYED_BODY_POLLS,
        "a 300 ms body wait must park instead of polling every 10 ms: \
         {polls} pump turns (max {MAX_DELAYED_BODY_POLLS})"
    );

    peer.join().expect("peer thread");
}
