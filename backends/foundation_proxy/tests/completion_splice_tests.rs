//! io_uring proxy relay: two Completion sockets spliced via `aio::splice` (F70 E1).
//!
//! WHY: The individual pieces (connect_completion, SEND_ZC, reactor wait) are each
//! tested in isolation. This proves the assembled relay — bytes flow both ways
//! between two Completion-backed sockets through the production async splice path
//! (the same `aio::splice` the tunnel, the upgrade relay and the raw TCP
//! passthrough use). The old polling `splice_bidirectional` slept 1 ms when idle;
//! the async splice parks on the valtron timer for a completion socket.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{sleep_async, valtron_test};
use foundation_http::native::writer::WriterConfig;
use foundation_iogate::{init_reactor_for, CompletionSocket, ServerIo};
use foundation_nativeapis::native::fd::Reactor;
use foundation_nativeapis::Token;
use foundation_netio::netcap::{Connection, RawStream};
use foundation_proxy::aio::splice;
use futures_util::future::{select, Either};

/// Wrap an accepted socket as a completion-backed connection the splice reads.
fn completion_stream(tcp: TcpStream, token: usize) -> SharedByteBufferStream<RawStream> {
    tcp.set_nonblocking(true).expect("nonblocking");
    let reactor = Reactor::get().expect("reactor");
    let socket = CompletionSocket::completion(tcp, reactor.registry(), Token(token), None)
        .expect("register completion socket");
    let raw =
        RawStream::from_connection(Connection::Completion(Box::new(socket))).expect("raw stream");
    SharedByteBufferStream::rwrite(raw)
}

#[valtron_test(seed = 70, threads = 2, timeout = 60000)]
async fn completion_splice_relays_bytes_both_directions() {
    if init_reactor_for(ServerIo::Completion).is_err() {
        eprintln!("io_uring unavailable — skipping completion splice test");
        return;
    }

    let server_b = TcpListener::bind("127.0.0.1:0").expect("bind server B");
    let addr_b = server_b.local_addr().expect("addr B");
    let server_d = TcpListener::bind("127.0.0.1:0").expect("bind server D");
    let addr_d = server_d.local_addr().expect("addr D");

    // The two outside peers run on plain threads with read timeouts.
    let peers = std::thread::spawn(move || {
        let mut a = TcpStream::connect(addr_b).expect("connect A");
        a.set_read_timeout(Some(Duration::from_secs(10))).ok();
        let mut c = TcpStream::connect(addr_d).expect("connect C");
        c.set_read_timeout(Some(Duration::from_secs(10))).ok();

        // Write A -> B, spliced B -> D, read C.
        a.write_all(b"hello from A").expect("write A");
        let mut buf = [0u8; 64];
        let n = c.read(&mut buf).expect("read C");
        assert_eq!(&buf[..n], b"hello from A");

        // Write C -> D, spliced D -> B, read A.
        c.write_all(b"response from C").expect("write C");
        let n = a.read(&mut buf).expect("read A");
        assert_eq!(&buf[..n], b"response from C");
        // Both sockets close here; the splice must end.
    });

    // The peers have connected (or will), so each accept returns at once.
    let (b, _) = server_b.accept().expect("accept B");
    let (d, _) = server_d.accept().expect("accept D");
    let relay = Box::pin(splice(
        completion_stream(b, 2),
        completion_stream(d, 3),
        WriterConfig::default(),
    ));
    let deadline = Box::pin(sleep_async(Duration::from_secs(30)));
    match select(relay, deadline).await {
        Either::Left(_) => {}
        Either::Right(_) => panic!("splice did not end after both peers closed"),
    }
    peers.join().expect("peers finished");
}
