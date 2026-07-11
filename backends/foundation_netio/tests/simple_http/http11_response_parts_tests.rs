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

use bytes::Bytes;
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
    let head =
        SimpleResponse::<()>::no_body(Status::OK, headers(&[("content-type", "text/plain")]));
    assert_eq!(
        render(Http11::response_head(head)),
        "HTTP/1.1 200 Ok\r\ncontent-type: text/plain\r\n\r\n"
    );
}

#[test]
fn chunked_body_chunk_carries_transfer_framing() {
    // "hello" is 5 bytes -> "5\r\nhello\r\n". The framing lives in the part iterator.
    assert_eq!(
        render(Http11::response_body_chunk(Http11Chunk::Chunked(
            b"hello".to_vec()
        ))),
        "5\r\nhello\r\n"
    );
    // 16-byte payload uses hex length (0x10 = "10").
    let sixteen = vec![b'x'; 16];
    let rendered = render(Http11::response_body_chunk(Http11Chunk::Chunked(sixteen)));
    assert!(
        rendered.starts_with("10\r\n"),
        "chunk length is hex: {rendered:?}"
    );
}

#[test]
fn raw_body_chunk_passes_through_unframed() {
    assert_eq!(
        render(Http11::response_body_chunk(Http11Chunk::Raw(
            b"hello".to_vec()
        ))),
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
    wire.push_str(&render(Http11::response_head(
        SimpleResponse::<()>::no_body(Status::OK, headers(&[("transfer-encoding", "chunked")])),
    )));
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

// ── Http11Chunk wire-rendering tests ─────────────────────────────────────────
//
// NOTE: `shared::Extensions` resolves to the type-erased struct from
// `shared/extensions.rs` via the glob-import, not the `Vec<(K, V)>` type alias
// in `impls.rs`. Use the explicit type for chunk-ext values.

#[test]
fn chunked_with_extensions_renders_correctly() {
    let chunk = Http11Chunk::ChunkedExt(
        b"data".to_vec(),
        vec![("key1".to_string(), Some("val1".to_string()))],
    );
    assert_eq!(
        String::from_utf8(chunk.render()).unwrap(),
        "4;key1=val1\r\ndata\r\n"
    );
}

#[test]
fn chunked_with_multiple_extensions() {
    let exts: Vec<(String, Option<String>)> = vec![
        ("a".into(), Some("1".into())),
        ("b".into(), Some("2".into())),
        ("c".into(), None),
    ];
    let chunk = Http11Chunk::ChunkedExt(b"hello".to_vec(), exts);
    assert_eq!(
        String::from_utf8(chunk.render()).unwrap(),
        "5;a=1;b=2;c\r\nhello\r\n"
    );
}

#[test]
fn chunked_with_empty_extensions_renders_same_as_plain_chunked() {
    let plain = Http11Chunk::Chunked(b"xyz".to_vec());
    let with_ext = Http11Chunk::ChunkedExt(b"xyz".to_vec(), vec![]);
    assert_eq!(plain.render(), with_ext.render());
}

#[test]
fn chunked_ext_renders_hex_length() {
    let data = vec![b'A'; 31];
    let chunk = Http11Chunk::ChunkedExt(data, vec![]);
    let rendered = String::from_utf8(chunk.render()).unwrap();
    assert!(
        rendered.starts_with("1f\r\n"),
        "hex length is 1f: {rendered:?}"
    );
    assert!(rendered.ends_with("\r\n"), "terminated with CRLF");
}

// ── Http11 request body renderer tests ──────────────────────────────────────

/// Build a basic `SimpleIncomingRequest` with a given body.
fn request_with_body(body: SendSafeBody) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_method(SimpleMethod::POST)
        .with_url(SimpleUrl::url_only("/test".to_string()))
        .with_proto(Proto::HTTP11)
        .with_some_body(Some(body))
        .build()
        .expect("build request")
}

/// Collect all chunks from a body renderer into a `Vec<u8>`.
fn collect_body(renderer: Http11RequestBodyIterator) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in renderer {
        out.extend_from_slice(&chunk.expect("render chunk"));
    }
    out
}

#[test]
fn body_streaming_yields_chunked_framed_output() {
    use foundation_core::valtron::PipeSender;
    let (producer, body) = pushable_request_body_with_depth(4);
    producer
        .try_push(bytes::Bytes::from_static(b"abc"))
        .unwrap();
    producer
        .try_push(bytes::Bytes::from_static(b"12345"))
        .unwrap();
    drop(producer); // close → consumer sees EOF

    let request = request_with_body(body);
    let output = collect_body(Http11RequestBodyIterator::new(request));

    let text = String::from_utf8(output).expect("valid UTF-8");
    assert_eq!(
        text, "3\r\nabc\r\n5\r\n12345\r\n0\r\n\r\n",
        "Stream body must be rendered with chunked transfer encoding framing and terminating chunk"
    );
}

#[test]
fn body_streaming_empty_stream_yields_only_terminator() {
    let (_producer, body) = pushable_request_body_with_depth(4);
    drop(_producer); // close → consumer sees EOF immediately

    let request = request_with_body(body);
    let output = collect_body(Http11RequestBodyIterator::new(request));

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "0\r\n\r\n",
        "empty Stream must yield just the terminating chunk"
    );
}

#[test]
fn body_streaming_with_retry_skips_empty() {
    use foundation_core::valtron::PipeSender;
    let (producer, body) = pushable_request_body_with_depth(4);
    // No data yet — consumer gets Retry. Eventually push one chunk.
    std::thread::sleep(std::time::Duration::from_millis(1));
    producer.try_push(bytes::Bytes::from_static(b"ok")).unwrap();
    drop(producer);

    let request = request_with_body(body);
    let output = collect_body(Http11RequestBodyIterator::new(request));

    let text = String::from_utf8(output).unwrap();
    assert_eq!(
        text, "2\r\nok\r\n0\r\n\r\n",
        "body with data should get chunk framing + terminator"
    );
}

#[test]
fn body_non_stream_types_are_unframed() {
    // Bytes body: no chunked framing.
    let req = request_with_body(SendSafeBody::Bytes(b"raw".to_vec()));
    let output = collect_body(Http11RequestBodyIterator::new(req));
    assert_eq!(
        output, b"raw",
        "Bytes body should be rendered as-is, no framing"
    );

    // Text body: no chunked framing.
    let req = request_with_body(SendSafeBody::Text("text".into()));
    let output = collect_body(Http11RequestBodyIterator::new(req));
    assert_eq!(
        output, b"text",
        "Text body should be rendered as-is, no framing"
    );

    // None body.
    let req = request_with_body(SendSafeBody::None);
    let output = collect_body(Http11RequestBodyIterator::new(req));
    assert!(output.is_empty(), "None body should be empty");
}
