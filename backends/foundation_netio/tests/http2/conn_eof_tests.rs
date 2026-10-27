//! A peer that closes the connection must end an `H2Conn`, not park it.
//!
//! `H2Conn` peeks for whole frames before reading them. The peek used to
//! answer `Ok(0)` at end of stream — the same as "not buffered yet" — so a
//! server whose client went away (or half-closed between frames) waited for
//! ever instead of noticing the disconnect.

use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::netcap::RawStream;

fn server_side_of(client_bytes: &[u8]) -> H2Conn {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut client = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
    let (server, _) = listener.accept().expect("accept");
    client.write_all(client_bytes).expect("write");
    drop(client); // the peer closes
    H2Conn::new_server(SharedByteBufferStream::rwrite(
        RawStream::from_tcp(server).expect("raw stream"),
    ))
}

#[test]
fn a_peer_that_closes_before_the_preface_ends_the_handshake() {
    let mut conn = server_side_of(b"");
    let err = conn.server_handshake().expect_err("the peer is gone");
    assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof, "{err}");
}

#[test]
fn a_peer_that_closes_mid_preface_ends_the_handshake() {
    let mut conn = server_side_of(b"PRI * HTTP/2.0\r\n");
    let err = conn.server_handshake().expect_err("the peer is gone");
    assert_ne!(
        err.kind(),
        io::ErrorKind::WouldBlock,
        "a closed peer must not read like 'not yet': {err}"
    );
}
