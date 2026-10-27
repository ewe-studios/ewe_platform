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

/// A socket that takes at most `per_write` bytes per call, and every other
/// call answers `WouldBlock` — a non-blocking socket whose send buffer fills.
struct Trickle {
    written: Vec<u8>,
    per_write: usize,
    block_next: bool,
}

impl io::Read for Trickle {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

impl io::Write for Trickle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let block = self.block_next;
        self.block_next = !block;
        if block {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = buf.len().min(self.per_write);
        self.written.extend_from_slice(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A flush interrupted by `WouldBlock` resumes where it stopped: every byte is
/// written exactly once. It used to `write_all` the whole buffer again, so the
/// prefix written before the `WouldBlock` went out twice.
#[test]
fn a_flush_interrupted_by_would_block_resumes_without_repeating_bytes() {
    use foundation_netio::http2::connection::H2Connection;

    let socket = Trickle {
        written: Vec::new(),
        per_write: 7,
        block_next: false,
    };
    let mut conn = H2Connection::new(socket, true);
    let frames: Vec<u8> = (0u8..=200).collect();
    conn.write_buf_mut().extend_from_slice(&frames);

    let mut attempts = 0;
    loop {
        attempts += 1;
        assert!(attempts < 1000, "flush never finished");
        match conn.flush() {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                assert!(conn.has_pending_writes(), "a WouldBlock leaves the rest queued");
            }
            Err(e) => panic!("flush failed: {e}"),
        }
    }
    assert!(!conn.has_pending_writes());
    assert_eq!(conn.socket_ref().written, frames, "every byte once, in order");
}
