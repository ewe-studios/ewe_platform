//! Regression tests for issue #1141: a gzip response body must be decoded
//! before a typed client reads it.
//!
//! WHY: a local Cloudflare Worker answers a typed unary call with HTTP 200,
//! `Content-Encoding: gzip`, and a gzip body. The JSON codec received the
//! compressed bytes and failed with "expected value at line 1 column 1". The
//! client advertised the encodings but never decoded them.
//!
//! WHAT: one test sends a gzipped JSON response from a local server through the
//! real client and asserts the typed JSON decode succeeds. One asserts a body
//! that expands past the configured limit is a typed error, not a truncated
//! body. One asserts an unknown encoding is also a typed error.

// The tests build gzip fixtures with flate2, which is optional in this crate.
#![cfg(feature = "gzip")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use flate2::write::GzEncoder;
use flate2::Compression;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::Stream;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::body_reader::{
    collect_bytes_from_send_safe, SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::client::{CompressionConfig, HttpClient};
use foundation_netio::shared::http::{
    HttpClientError, HttpReaderError, HttpResponseReader, IncomingResponseParts, SendSafeBody,
    SimpleHeader, SimpleHttpBody, Status,
};
use foundation_netio::PreparedRequestBuilder;

/// The typed JSON answer of the test server.
#[derive(serde::Deserialize)]
struct Container {
    id: String,
    state: String,
}

#[derive(serde::Deserialize)]
struct Reply {
    container: Container,
}

/// Compress `data` as a gzip member.
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("write the gzip body");
    encoder.finish().expect("finish the gzip body")
}

/// Serve one HTTP response on a loopback port and return the URL.
///
/// The thread accepts one connection, reads the request head, writes `response`
/// and returns the request bytes so a test can assert on them.
fn serve_once(response: Vec<u8>) -> (String, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let addr = listener.local_addr().expect("read the local address");
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept the request");
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let read = socket.read(&mut chunk).expect("read the request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        socket.write_all(&response).expect("write the response");
        socket.flush().expect("flush the response");
        request
    });
    (format!("http://{addr}/"), handle)
}

/// Build a raw HTTP/1.1 response with a fixed-length body.
fn response_with_body(content_encoding: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json\r\n\
         Content-Encoding: {content_encoding}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/// A gzipped JSON answer reaches the typed codec as plain JSON.
#[test]
fn gzip_json_response_decodes_for_the_typed_client() {
    // The blocking client drives its exchange on the valtron pool.
    let _pool_guard = foundation_core::valtron::initialize_pool(1141, None);

    let json = br#"{"container":{"id":"abc-123","state":"running"}}"#;
    let (url, server) = serve_once(response_with_body("gzip", &gzip(json)));

    let client = NativeHttpClient::from_system();
    let request = PreparedRequestBuilder::get(&url)
        .expect("build the request URL")
        .build();
    let response = client.send(request).expect("send the request");
    let request_bytes = server.join().expect("join the test server");

    // The client advertises the encodings it decodes, so the test server is
    // allowed to compress. A caller-set header would win over this default.
    let request_text = String::from_utf8_lossy(&request_bytes).to_ascii_lowercase();
    assert!(
        request_text.contains("accept-encoding: br, gzip, deflate"),
        "the request must advertise gzip, got: {request_text}"
    );

    assert_eq!(response.get_status(), Status::OK);
    // The header described the wire bytes; after decoding it must not reach
    // the codec, or a protocol layer would decompress the body a second time.
    assert!(
        !response
            .get_headers_ref()
            .contains_key(&SimpleHeader::CONTENT_ENCODING),
        "the decoded response must not carry Content-Encoding"
    );
    assert!(
        !response
            .get_headers_ref()
            .contains_key(&SimpleHeader::CONTENT_LENGTH),
        "the decoded response must not carry the compressed Content-Length"
    );

    let body = collect_bytes_from_send_safe(response.take_body());

    let reply: Reply =
        serde_json::from_slice(&body).expect("the typed JSON decode after gzip decoding");
    assert_eq!(reply.container.id, "abc-123");
    assert_eq!(reply.container.state, "running");
}

/// Read the one body part from a response reader, ignoring the intro and
/// headers.
fn read_body(reader: HttpResponseReader<SimpleHttpBody, std::io::Cursor<Vec<u8>>>) -> SendSafeBody {
    let mut body = None;
    for part in reader {
        match part.expect("parse the response head") {
            IncomingResponseParts::StreamedBody(payload)
            | IncomingResponseParts::SizedBody(payload) => {
                body = Some(payload);
                break;
            }
            IncomingResponseParts::NoBody => panic!("the test response has a body"),
            IncomingResponseParts::Intro(..)
            | IncomingResponseParts::Headers(_)
            | IncomingResponseParts::SKIP => {}
        }
    }
    body.expect("a response body")
}

/// Pull the first typed error from a body iterator.
fn first_body_error(body: SendSafeBody) -> Box<dyn std::error::Error + 'static> {
    for item in SendSafeBodyBytesIterator::new(body) {
        if let Stream::Next(SendSafeBodyBytesItem::StreamError(err)) = item {
            return err;
        }
    }
    panic!("the body must fail with a typed error");
}

/// A decoded body past the configured cap fails with a typed error, and is not
/// truncated to the cap.
#[test]
fn decoded_body_past_the_limit_is_a_typed_error() {
    let original = vec![b'a'; 4096];
    let compressed = gzip(&original);
    let raw = response_with_body("gzip", &compressed);

    let config = SimpleHttpBody::default()
        .with_decompression(CompressionConfig::default().with_max_decompressed_size(64));
    let stream = SharedByteBufferStream::rwrite(std::io::Cursor::new(raw));
    let reader = HttpResponseReader::<SimpleHttpBody, _>::new(stream, config);

    let err = first_body_error(read_body(reader));
    match err.downcast_ref::<HttpReaderError>() {
        Some(HttpReaderError::DecompressedBodyTooLarge(limit)) => {
            assert_eq!(*limit, 64, "the error names the configured limit");
        }
        other => panic!("expected DecompressedBodyTooLarge, got {other:?}: {err}"),
    }
}

/// An unknown encoding fails with a typed error that names it; compressed bytes
/// never reach a codec.
#[test]
fn unknown_content_encoding_is_a_typed_error_naming_the_encoding() {
    let raw = response_with_body("zstd", b"compressed");

    let stream = SharedByteBufferStream::rwrite(std::io::Cursor::new(raw));
    let reader = HttpResponseReader::<SimpleHttpBody, _>::new(stream, SimpleHttpBody::default());

    // The body cannot be built, so the reader reports the typed error on the
    // body part.
    let mut typed = None;
    for part in reader {
        match part {
            Ok(
                IncomingResponseParts::Intro(..)
                | IncomingResponseParts::Headers(_)
                | IncomingResponseParts::SKIP,
            ) => {}
            Ok(other) => panic!("expected a typed body error, got {other:?}"),
            Err(HttpReaderError::BodyBuildFailed(err)) => {
                if let Some(HttpReaderError::UnsupportedContentEncoding(name)) =
                    err.downcast_ref::<HttpReaderError>()
                {
                    typed = Some(name.clone());
                }
                break;
            }
            Err(err) => panic!("expected BodyBuildFailed, got {err:?}"),
        }
    }
    assert_eq!(typed.as_deref(), Some("zstd"));

    // The same decision is visible at the client error layer.
    let client_error: HttpClientError =
        HttpClientError::ReaderError(HttpReaderError::UnsupportedContentEncoding("zstd".into()));
    assert!(client_error.to_string().contains("zstd"));
}
