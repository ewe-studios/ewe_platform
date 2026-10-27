//! `Transfer-Encoding` codings are case-insensitive (RFC 9112 §7): a message
//! whose last coding is spelled `Chunked` is still chunked. The readers used to
//! find chunked's position with a case-sensitive compare, so `deflate, Chunked`
//! was rejected as an unknown coding.

use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    http_streams, HttpReaderError, IncomingRequestParts, IncomingResponseParts, SendSafeBody,
};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread;

/// Accept one connection on a fresh listener after a peer writes `message`.
fn stream_with(message: &'static str) -> RawStream {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let writer = thread::spawn(move || {
        let mut client = TcpStream::connect(addr).expect("connect");
        client.write_all(message.as_bytes()).expect("write");
        // Keep the socket open until the reader is done with it.
        client
    });
    let (server, _) = listener.accept().expect("accept");
    drop(writer.join().expect("writer"));
    RawStream::from_tcp(server).expect("raw stream")
}

#[test]
fn request_reader_accepts_mixed_case_chunked_as_last_coding() {
    let reader = http_streams::send::request_reader(stream_with(
        "POST /url HTTP/1.1\r\nTransfer-Encoding: deflate\r\nTransfer-Encoding: Chunked\r\n\r\n5\r\nWorld\r\n0\r\n\r\n",
    ));
    let parts = reader
        .into_iter()
        .collect::<Result<Vec<IncomingRequestParts>, HttpReaderError>>()
        .expect("a mixed-case chunked coding is valid");
    assert!(
        matches!(
            parts.last(),
            Some(IncomingRequestParts::StreamedBody(
                SendSafeBody::ChunkedStream(Some(_))
            ))
        ),
        "the body should be read as chunked, got {parts:?}"
    );
}

#[test]
fn response_reader_accepts_mixed_case_chunked_as_last_coding() {
    let reader = http_streams::send::response_reader(stream_with(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\nTransfer-Encoding: CHUNKED\r\n\r\n5\r\nWorld\r\n0\r\n\r\n",
    ));
    let parts = reader
        .into_iter()
        .collect::<Result<Vec<IncomingResponseParts>, HttpReaderError>>()
        .expect("a mixed-case chunked coding is valid");
    assert!(
        matches!(
            parts.last(),
            Some(IncomingResponseParts::StreamedBody(
                SendSafeBody::ChunkedStream(Some(_))
            ))
        ),
        "the body should be read as chunked, got {parts:?}"
    );
}
