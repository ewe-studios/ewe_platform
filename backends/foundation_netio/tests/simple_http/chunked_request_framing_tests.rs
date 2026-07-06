//! Streaming bodies must declare their transfer framing in the head.
//!
//! WHY: `Http11RequestBodyIterator` frames every streaming request body
//! (`Stream`/`ChunkedStream`/`LineFeedStream`) as chunked transfer-coding, but
//! for a long time the head advertised neither `Content-Length` nor
//! `Transfer-Encoding`. A receiver then finds no framing header, treats the
//! request as bodyless (the request parser's fallthrough state), drops the
//! chunked bytes, and mishandles the `Expect: 100-continue` handshake.
//!
//! WHAT: the request/response builders (and the redirect send path) now inject
//! `Transfer-Encoding: chunked` — and drop any stale `Content-Length` — for the
//! bodies the renderer chunk-frames. These tests pin that contract and prove a
//! real server request reader now reads the body instead of `NoBody`.
//!
//! HOW: builder-level header assertions + a render→parse round trip through
//! `HTTPStreams::next_request()`.

use bytes::Bytes;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::url::Uri;
use foundation_core::valtron::PipeSender;
use foundation_netio::simple_http::shared::{
    ensure_chunked_transfer_encoding, pushable_request_body_with_depth, Http11, HTTPStreams,
    IncomingRequestParts, Proto, RenderHttp, SendSafeBody, SimpleHeader, SimpleHeaders,
    SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse, SimpleUrl, Status,
};

const CHUNKED: &str = "chunked";

fn has_chunked_te(headers: &SimpleHeaders) -> bool {
    headers
        .get(&SimpleHeader::TRANSFER_ENCODING)
        .is_some_and(|values| values.iter().any(|v| v.eq_ignore_ascii_case(CHUNKED)))
}

fn build_request(body: SendSafeBody) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_method(SimpleMethod::POST)
        .with_uri(Uri::parse("http://localhost/echo").expect("uri"))
        .with_url(SimpleUrl::url_only("http://localhost/echo".to_string()))
        .with_proto(Proto::HTTP11)
        .with_some_body(Some(body))
        .build()
        .expect("build request")
}

// ── request builder: streaming variants declare chunked framing ────────────

#[test]
fn request_stream_body_declares_chunked_te_and_no_content_length() {
    let req = build_request(SendSafeBody::Stream(None));
    assert!(has_chunked_te(&req.headers), "Stream body must declare TE: chunked");
    assert!(
        !req.headers.contains_key(&SimpleHeader::CONTENT_LENGTH),
        "chunked body must not carry Content-Length"
    );
}

#[test]
fn request_chunked_stream_body_declares_chunked_te() {
    let req = build_request(SendSafeBody::ChunkedStream(None));
    assert!(has_chunked_te(&req.headers), "ChunkedStream body must declare TE: chunked");
    assert!(!req.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
}

#[test]
fn request_linefeed_stream_body_declares_chunked_te() {
    let req = build_request(SendSafeBody::LineFeedStream(None));
    assert!(has_chunked_te(&req.headers), "LineFeedStream body must declare TE: chunked");
    assert!(!req.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
}

#[test]
fn request_bytes_body_declares_content_length_not_te() {
    let req = build_request(SendSafeBody::Bytes(b"hello".to_vec()));
    assert!(
        req.headers.contains_key(&SimpleHeader::CONTENT_LENGTH),
        "fixed-size Bytes body must declare Content-Length"
    );
    assert!(!has_chunked_te(&req.headers), "fixed-size body must not be chunked");
}

#[test]
fn request_stale_content_length_is_dropped_for_stream_body() {
    // A caller that mistakenly set Content-Length on a streaming body: the
    // renderer will chunk-frame regardless, so the head must be corrected.
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::CONTENT_LENGTH, vec!["17".to_string()]);
    let req = SimpleIncomingRequest::builder()
        .with_method(SimpleMethod::POST)
        .with_uri(Uri::parse("http://localhost/echo").expect("uri"))
        .with_url(SimpleUrl::url_only("http://localhost/echo".to_string()))
        .with_proto(Proto::HTTP11)
        .with_headers(headers)
        .with_some_body(Some(SendSafeBody::Stream(None)))
        .build()
        .expect("build request");

    assert!(has_chunked_te(&req.headers));
    assert!(
        !req.headers.contains_key(&SimpleHeader::CONTENT_LENGTH),
        "stale Content-Length must be removed (RFC 7230 §3.3.3: TE and CL are exclusive)"
    );
}

// ── response builder: only ChunkedStream is chunk-framed ────────────────────

#[test]
fn response_chunked_stream_declares_chunked_te() {
    let resp = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .with_body(SendSafeBody::ChunkedStream(None))
        .build()
        .expect("build response");
    assert!(has_chunked_te(&resp.headers), "chunked response must declare TE: chunked");
    assert!(!resp.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
}

#[test]
fn response_raw_stream_declares_no_framing_header() {
    // Raw `Stream`/`LineFeedStream` responses are delimited by connection close,
    // not chunked framing — they must NOT advertise Transfer-Encoding.
    let resp = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .with_body(SendSafeBody::Stream(None))
        .build()
        .expect("build response");
    assert!(!has_chunked_te(&resp.headers), "raw Stream response must not declare TE: chunked");
    assert!(!resp.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
}

// ── helper is idempotent ────────────────────────────────────────────────────

#[test]
fn ensure_chunked_transfer_encoding_is_idempotent() {
    let mut headers = SimpleHeaders::new();
    ensure_chunked_transfer_encoding(&mut headers);
    ensure_chunked_transfer_encoding(&mut headers);
    let values = headers
        .get(&SimpleHeader::TRANSFER_ENCODING)
        .expect("TE present");
    assert_eq!(values, &vec![CHUNKED.to_string()], "chunked must not be duplicated");
}

// ── the bug, end to end: a real server reader reads the body, not NoBody ─────

#[test]
fn server_reads_chunked_request_body_instead_of_nobody() {
    // Client side: build a streaming POST and render exactly what goes on the
    // wire — head (from the builder-produced headers) + chunked body.
    let (producer, body) = pushable_request_body_with_depth(4);
    producer.try_push(Bytes::from_static(b"hello")).expect("push");
    producer.try_push(Bytes::from_static(b"world")).expect("push");
    drop(producer); // close → renderer emits the terminating chunk

    let req = build_request(body);

    // Sanity: the head the builder produced advertises chunked framing.
    assert!(has_chunked_te(&req.headers));

    let head = Http11::request_descriptor(req.descriptor())
        .http_render_string()
        .expect("render head");
    let body_bytes = {
        let mut out = Vec::new();
        for chunk in foundation_netio::simple_http::shared::Http11RequestBodyIterator::new(req) {
            out.extend_from_slice(&chunk.expect("render chunk"));
        }
        out
    };

    let mut wire = head.into_bytes();
    wire.extend_from_slice(&body_bytes);

    // Server side: feed the exact bytes to the request reader.
    let streams = HTTPStreams::new(SharedByteBufferStream::rwrite(std::io::Cursor::new(wire)));
    let reader = streams.next_request();

    let mut saw_streamed_body = false;
    let mut saw_nobody = false;
    for part in reader {
        match part.expect("parse part") {
            IncomingRequestParts::StreamedBody(_) => saw_streamed_body = true,
            IncomingRequestParts::NoBody => saw_nobody = true,
            _ => {}
        }
    }

    assert!(
        saw_streamed_body,
        "server must read the chunked request body (StreamedBody), not treat it as bodyless"
    );
    assert!(
        !saw_nobody,
        "a chunk-framed request with Transfer-Encoding: chunked must never parse as NoBody"
    );
}
