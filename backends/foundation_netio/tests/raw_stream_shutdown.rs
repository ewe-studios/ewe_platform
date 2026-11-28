//! `RawStream::shutdown` sends a FIN at once, before the stream is dropped.
//!
//! Issue #1246: a response that ends the connection must not wait for every
//! shared handle to drop before the client sees end-of-stream.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

use foundation_netio::netcap::RawStream;

#[test]
fn shutdown_write_sends_a_fin_before_the_drop() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local address");
    let mut client = TcpStream::connect(addr).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    let mut raw = RawStream::from_tcp(server).expect("raw stream");

    raw.write_all(b"bye").expect("write");
    raw.flush().expect("flush");
    raw.shutdown(Shutdown::Write).expect("shutdown");

    // `raw` is still alive: the end-of-stream must come from the shutdown, not
    // from the drop below. A missing FIN blocks until the read timeout.
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut buf = [0u8; 16];
    let mut seen = Vec::new();
    loop {
        let n = client.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        seen.extend_from_slice(&buf[..n]);
    }
    assert_eq!(seen, b"bye");

    drop(raw);
}
