//! An idle HTTP/2 connection task parks on its socket instead of polling.
//!
//! `H2ConnectionHandler` used to re-poll every 10 ms (1 ms during the
//! handshake) whenever no frame was buffered — the "47b" reactor-parking gap.
//! It now registers the socket with the shared reactor and returns
//! `TaskStatus::Depends`, resuming when the socket, a handler's response pipe,
//! the idle deadline or shutdown says so.

#![cfg(all(unix, not(target_family = "wasm")))]

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::{OnSignal, WaitGroup};
use foundation_core::valtron::{Ctx, TaskIterator, TaskStatus};
use foundation_http::native::server::H2ConnectionHandler;
use foundation_http::shared::app::HttpApp;
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::http2::connection::CLIENT_PREFACE;
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
        Arc::new(OnSignal::new()),
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
