//! Feature 05 (Decision 12 §1–§3): HTTP/1.1 per-part response iterators + trailers.
//!
//! These tests exercise the new `Http11` per-part variants a streaming handler
//! composes (status line / headers / head / body chunk / trailers), the chunked
//! wire framing living in the part iterator, and the `trailers` field on
//! `SimpleOutgoingResponse`. The atomic whole-response path is covered (unchanged)
//! by the existing simple_http suite.
//!
//! Header keys use `SimpleHeader::Custom` with explicit strings so the rendered
//! bytes are exactly as written — independent of feature 06's known-variant
//! lowercase rendering.

use foundation_netio::simple_http::shared::*;

/// Collect a single `Http11` part's rendered wire bytes into a `String`.
fn render(part: Http11) -> String {
    part.http_render_string().expect("part renders")
}

/// Build a `SimpleHeaders` from `(name, value)` pairs (custom, case-preserved).
fn headers(pairs: &[(&str, &str)]) -> SimpleHeaders {
    let mut map = SimpleHeaders::new();
    for (name, value) in pairs {
        map.entry(SimpleHeader::Custom((*name).into()))
            .or_default()
            .push((*value).to_string());
    }
    map
}

#[test]
fn response_status_line_part() {
    assert_eq!(
        render(Http11::response_status_line(Status::OK)),
        "HTTP/1.1 200 Ok\r\n"
    );
}

#[test]
fn response_headers_part_ends_with_crlf() {
    let block = render(Http11::response_headers(headers(&[(
        "content-type",
        "text/plain",
    )])));
    assert_eq!(block, "content-type: text/plain\r\n\r\n");
}

#[test]
fn response_head_part_is_status_line_plus_headers() {
    let head = SimpleResponse::<()>::no_body(Status::OK, headers(&[("content-type", "text/plain")]));
    assert_eq!(
        render(Http11::response_head(head)),
        "HTTP/1.1 200 Ok\r\ncontent-type: text/plain\r\n\r\n"
    );
}

#[test]
fn chunked_body_chunk_carries_transfer_framing() {
    // "hello" is 5 bytes -> "5\r\nhello\r\n". The framing lives in the part iterator.
    assert_eq!(
        render(Http11::response_body_chunk(Http11Chunk::Chunked(b"hello".to_vec()))),
        "5\r\nhello\r\n"
    );
    // 16-byte payload uses hex length (0x10 = "10").
    let sixteen = vec![b'x'; 16];
    let rendered = render(Http11::response_body_chunk(Http11Chunk::Chunked(sixteen)));
    assert!(rendered.starts_with("10\r\n"), "chunk length is hex: {rendered:?}");
}

#[test]
fn raw_body_chunk_passes_through_unframed() {
    assert_eq!(
        render(Http11::response_body_chunk(Http11Chunk::Raw(b"hello".to_vec()))),
        "hello"
    );
}

#[test]
fn trailers_part_emits_terminator_and_block() {
    // Non-empty trailers: "0\r\n<trailer lines>\r\n".
    let with_trailers = render(Http11::response_trailers(headers(&[("grpc-status", "0")])));
    assert_eq!(with_trailers, "0\r\ngrpc-status: 0\r\n\r\n");

    // Empty trailers: the bare chunked terminator.
    assert_eq!(
        render(Http11::response_trailers(SimpleHeaders::new())),
        "0\r\n\r\n"
    );
}

/// Acceptance: a streaming handler emits head -> N flushed chunks -> trailers.
#[test]
fn streaming_handler_composes_head_chunks_trailers() {
    let mut wire = String::new();
    wire.push_str(&render(Http11::response_head(SimpleResponse::<()>::no_body(
        Status::OK,
        headers(&[("transfer-encoding", "chunked")]),
    ))));
    wire.push_str(&render(Http11::response_body_chunk(Http11Chunk::Chunked(
        b"first".to_vec(),
    ))));
    wire.push_str(&render(Http11::response_body_chunk(Http11Chunk::Chunked(
        b"second".to_vec(),
    ))));
    wire.push_str(&render(Http11::response_trailers(SimpleHeaders::new())));

    assert_eq!(
        wire,
        "HTTP/1.1 200 Ok\r\n\
         transfer-encoding: chunked\r\n\r\n\
         5\r\nfirst\r\n\
         6\r\nsecond\r\n\
         0\r\n\r\n"
    );
}

/// Acceptance: interim 1xx responses are a `ResponseHead` emitted before the final.
#[test]
fn interim_1xx_head_precedes_final_head() {
    let interim = render(Http11::response_head(SimpleResponse::<()>::no_body(
        Status::Continue,
        SimpleHeaders::new(),
    )));
    let final_head = render(Http11::response_head(SimpleResponse::<()>::no_body(
        Status::OK,
        headers(&[("content-length", "0")]),
    )));

    // Header-less interim head is just the status line + terminating CRLF.
    assert_eq!(interim, "HTTP/1.1 100 Continue\r\n\r\n");
    assert_eq!(final_head, "HTTP/1.1 200 Ok\r\ncontent-length: 0\r\n\r\n");
}

/// The `trailers` field is carried on `SimpleOutgoingResponse` (empty by default).
#[test]
fn outgoing_response_carries_trailers_field() {
    let default_response = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .add_header(SimpleHeader::CONTENT_LENGTH, "0")
        .build()
        .expect("build");
    assert!(
        default_response.trailers.is_empty(),
        "trailers default to empty"
    );

    let grpc_status = SimpleHeader::Custom("grpc-status".into());
    let with_trailers = SimpleOutgoingResponse::builder()
        .with_status(Status::OK)
        .add_header(SimpleHeader::TRANSFER_ENCODING, "chunked")
        .add_trailer(grpc_status.clone(), "0")
        .build()
        .expect("build");
    assert_eq!(
        with_trailers.trailers.get(&grpc_status),
        Some(&vec!["0".to_string()])
    );
}
