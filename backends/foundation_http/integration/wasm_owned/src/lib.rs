//! The foundation_http pure-Rust wasm cases on the OWNED harness (spec 88
//! feature 02, T6).
//!
//! WHY: three native test files (`tests/wasm_response_tests.rs`,
//! `tests/wasm_stream_tests.rs`, `tests/wasm_server_tests.rs`) held these
//! assertions, and no CI run enabled `wasm-test` (#570). The wasm-bindgen crate
//! (`integration/wasm`) proves the bridge. This crate proves the same pure-Rust
//! types on Deno and Chromium with no wasm-bindgen glue (rules R9, R15).
//!
//! The cases live in the lib, not `tests/`: the owned runner builds the package
//! lib and reads its `__fwt_` exports (spec 88 section 5).

use std::io::Write;
use std::sync::Arc;

use foundation_context::{Ctx, MonotonicClock};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::shared::serve::ServeWriter;
use foundation_http::wasm::response::WasmResponse;
use foundation_http::wasm::serve_writer::run_middleware_writer;
use foundation_http::wasm::stream::{parse_raw, WasmStream};
use foundation_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use foundation_macros::wasm_test;

/// A deterministic context: the cases read no clock and draw no random value.
fn ctx() -> Ctx {
    Ctx::fixed(Arc::new(MonotonicClock::new()))
}

/// A valid GET request for the middleware cases.
fn make_request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_parsed_url("http://example.com/test")
        .with_method(SimpleMethod::from("GET".to_string()))
        .with_proto(Proto::HTTP11)
        .with_headers(SimpleHeaders::new())
        .build()
        .expect("valid request")
}

// ── Middleware used by the server cases ─────────────────────────────

struct PassThroughMiddleware;

impl RequestMiddleware for PassThroughMiddleware {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Continue
    }
}

struct RejectMiddleware;

impl RequestMiddleware for RejectMiddleware {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Response(SimpleOutgoingResponse {
            proto: Proto::HTTP11,
            status: Status::Unauthorized,
            headers: SimpleHeaders::new(),
            body: Some(SendSafeBody::Text("Unauthorized".into())),
            trailers: SimpleHeaders::new(),
        })
    }
}

struct AddHeaderMiddleware;

impl RequestMiddleware for AddHeaderMiddleware {
    fn handle(&self, _ctx: &Ctx, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        req.headers
            .entry(SimpleHeader::from("X-Middleware".to_string()))
            .or_default()
            .push("passed".to_string());
        MiddlewareResult::Continue
    }
}

// ── WasmResponse (tests/wasm_response_tests.rs) ─────────────────────

#[wasm_test]
fn test_wasm_response_default() {
    let resp = WasmResponse::default();
    assert_eq!(resp.status, 0);
    assert!(resp.body.is_none());
    assert!(resp.headers.is_empty());
}

#[wasm_test]
fn test_wasm_response_new() {
    let resp = WasmResponse::new(201);
    assert_eq!(resp.status, 201);
    assert!(resp.body.is_none());
    assert!(resp.headers.is_empty());
}

#[wasm_test]
fn test_wasm_response_with_body() {
    let resp = WasmResponse::new(200).with_body(b"hello");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, Some(b"hello".to_vec()));
}

#[wasm_test]
fn test_wasm_response_with_body_binary() {
    let data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01];
    let resp = WasmResponse::new(200).with_body(data.clone());
    assert_eq!(resp.body, Some(data));
}

#[wasm_test]
fn test_wasm_response_with_single_header() {
    let resp = WasmResponse::new(200).with_header("Content-Type", "application/json");
    assert_eq!(resp.headers.len(), 1);
    assert_eq!(
        resp.headers[0],
        ("Content-Type".into(), "application/json".into())
    );
}

#[wasm_test]
fn test_wasm_response_chained() {
    let resp = WasmResponse::new(201)
        .with_body(br#"{"id":1}"#)
        .with_header("Content-Type", "application/json")
        .with_header("X-Request-Id", "abc-123");
    assert_eq!(resp.status, 201);
    assert_eq!(resp.body, Some(br#"{"id":1}"#.to_vec()));
    assert_eq!(resp.headers.len(), 2);
}

#[wasm_test]
fn test_wasm_response_various_statuses() {
    let statuses = [200, 201, 204, 301, 302, 400, 401, 403, 404, 500, 502, 503];
    for status in statuses {
        let resp = WasmResponse::new(status);
        assert_eq!(resp.status, status);
    }
}

#[wasm_test]
fn test_wasm_response_empty_body() {
    let resp = WasmResponse::new(204).with_body(b"");
    assert_eq!(resp.body, Some(Vec::new()));
}

#[wasm_test]
fn test_wasm_response_multiple_same_header_name() {
    let resp = WasmResponse::new(200)
        .with_header("Set-Cookie", "a=1")
        .with_header("Set-Cookie", "b=2");
    assert_eq!(resp.headers.len(), 2);
    assert_eq!(resp.headers[0], ("Set-Cookie".into(), "a=1".into()));
    assert_eq!(resp.headers[1], ("Set-Cookie".into(), "b=2".into()));
}

// ── WasmStream (tests/wasm_stream_tests.rs) ─────────────────────────

#[wasm_test]
fn test_wasm_stream_new() {
    let stream = WasmStream::new();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(Vec::new()));
    assert!(response.headers.is_empty());
}

#[wasm_test]
fn test_wasm_stream_default() {
    let stream = WasmStream::default();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
}

#[wasm_test]
fn test_wasm_stream_write_returns_len() {
    let mut stream = WasmStream::new();
    let n = stream.write(b"test").unwrap();
    assert_eq!(n, 4);
}

#[wasm_test]
fn test_wasm_stream_write_all() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nhello").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"hello".to_vec()));
}

#[wasm_test]
fn test_wasm_stream_parse_200() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
}

#[wasm_test]
fn test_wasm_stream_parse_404() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 404);
}

#[wasm_test]
fn test_wasm_stream_parse_201() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 201 Created\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 201);
}

#[wasm_test]
fn test_wasm_stream_parse_500() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\nerror")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 500);
    assert_eq!(response.body, Some(b"error".to_vec()));
}

#[wasm_test]
fn test_wasm_stream_parse_301_with_location() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 301 Moved Permanently\r\nLocation: /new\r\n\r\n")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 301);
    assert_eq!(response.headers.len(), 1);
    assert_eq!(
        response.headers[0],
        ("Location".to_string(), "/new".to_string())
    );
}

#[wasm_test]
fn test_wasm_stream_full_response() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"hello".to_vec()));
    assert_eq!(response.headers.len(), 2);
    assert!(response
        .headers
        .iter()
        .any(|(k, v)| k == "Content-Type" && v == "text/plain"));
    assert!(response
        .headers
        .iter()
        .any(|(k, v)| k == "Content-Length" && v == "5"));
}

#[wasm_test]
fn test_wasm_stream_json_response() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(br#"{"ok":true}"#.to_vec()));
    assert_eq!(response.headers.len(), 1);
    assert_eq!(
        response.headers[0],
        ("Content-Type".to_string(), "application/json".to_string())
    );
}

#[wasm_test]
fn test_wasm_stream_multiple_set_cookie() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n\r\n")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.headers.len(), 2);
    assert_eq!(
        response.headers[0],
        ("Set-Cookie".to_string(), "a=1".to_string())
    );
    assert_eq!(
        response.headers[1],
        ("Set-Cookie".to_string(), "b=2".to_string())
    );
}

#[wasm_test]
fn test_wasm_stream_204_no_body() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 204);
    assert_eq!(response.body, Some(Vec::new()));
}

#[wasm_test]
fn test_wasm_stream_malformed_defaults_to_200() {
    let mut stream = WasmStream::new();
    stream.write_all(b"GARBAGE\r\n\r\nbody").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"body".to_vec()));
}

#[wasm_test]
fn test_wasm_stream_flush_triggers_parse() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 201 Created\r\n\r\n").unwrap();
    stream.flush().unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 201);
}

#[wasm_test]
fn test_wasm_stream_multiple_writes() {
    let mut stream = WasmStream::new();
    stream.write(b"HTTP/1.1 ").unwrap();
    stream.write(b"200 OK\r\n").unwrap();
    stream.write(b"\r\n").unwrap();
    stream.write(b"done").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"done".to_vec()));
}

#[wasm_test]
fn test_wasm_stream_into_bytes() {
    let mut stream = WasmStream::new();
    stream.write_all(b"raw bytes").unwrap();
    let bytes = stream.into_bytes();
    assert_eq!(bytes, b"raw bytes");
}

#[wasm_test]
fn test_wasm_stream_body_with_crlf_in_middle() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 200 OK\r\n\r\nline1\r\n\r\nline2")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"line1\r\n\r\nline2".to_vec()));
}

#[wasm_test]
fn test_wasm_stream_empty_buffer() {
    let stream = WasmStream::new();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(Vec::new()));
}

#[wasm_test]
fn test_wasm_stream_header_with_extra_space() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nX-Custom: value\r\n\r\n")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.headers.len(), 1);
    assert_eq!(
        response.headers[0],
        ("X-Custom".to_string(), "value".to_string())
    );
}

#[wasm_test]
fn test_wasm_stream_header_without_space_after_colon() {
    let mut stream = WasmStream::new();
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nBadHeader:value\r\n\r\n")
        .unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.headers.len(), 0);
}

#[wasm_test]
fn test_parse_raw_standalone() {
    let parsed = parse_raw(b"HTTP/1.1 418 I'm a teapot\r\nX-Teapot: yes\r\n\r\nbrewing");
    assert_eq!(parsed.status, 418);
    assert_eq!(parsed.headers.len(), 1);
    assert_eq!(
        parsed.headers[0],
        ("X-Teapot".to_string(), "yes".to_string())
    );
    assert_eq!(parsed.body, b"brewing");
}

// ── The middleware chain (tests/wasm_server_tests.rs) ───────────────

#[wasm_test]
fn test_run_middleware_no_chain_returns_none() {
    let app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    let (returned_req, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    assert!(short_circuit.is_none());
    assert_eq!(returned_req.method, SimpleMethod::from("GET".to_string()));
}

#[wasm_test]
fn test_run_middleware_pass_through() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(PassThroughMiddleware);
    let (returned_req, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    assert!(short_circuit.is_none());
    assert_eq!(returned_req.method, SimpleMethod::from("GET".to_string()));
}

#[wasm_test]
fn test_run_middleware_short_circuit_401() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(RejectMiddleware);
    let (_returned_req, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    let response = short_circuit.expect("should have short-circuited");
    assert_eq!(response.status, 401);
    assert_eq!(response.body, Some(b"Unauthorized".to_vec()));
    assert!(response.headers.is_empty());
}

#[wasm_test]
fn test_run_middleware_adds_header() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(AddHeaderMiddleware);
    let (returned_req, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    assert!(short_circuit.is_none());
    let header_key = SimpleHeader::from("X-Middleware".to_string());
    let values = returned_req
        .headers
        .get(&header_key)
        .expect("header should exist");
    assert_eq!(values[0], "passed");
}

#[wasm_test]
fn test_run_middleware_multiple_continue() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(PassThroughMiddleware);
    app.middleware(AddHeaderMiddleware);
    app.middleware(PassThroughMiddleware);
    let (returned_req, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    assert!(short_circuit.is_none());
    let header_key = SimpleHeader::from("X-Middleware".to_string());
    assert!(returned_req.headers.get(&header_key).is_some());
}

#[wasm_test]
fn test_run_middleware_first_blocks_skips_rest() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(RejectMiddleware);
    app.middleware(AddHeaderMiddleware);
    let (_returned_req, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    let response = short_circuit.expect("should have short-circuited");
    assert_eq!(response.status, 401);
    assert!(response.headers.iter().all(|(k, _)| k != "X-Middleware"));
}

#[wasm_test]
fn test_middleware_short_circuit_forbidden_text() {
    struct TextMiddleware;
    impl RequestMiddleware for TextMiddleware {
        fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
            MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::Forbidden,
                headers: SimpleHeaders::new(),
                body: Some(SendSafeBody::Text("Access denied".into())),
                trailers: SimpleHeaders::new(),
            })
        }
    }

    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(TextMiddleware);
    let (_, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    let response = short_circuit.unwrap();
    assert_eq!(response.status, 403);
    assert_eq!(response.body, Some(b"Access denied".to_vec()));
}

#[wasm_test]
fn test_middleware_short_circuit_bytes_body() {
    struct BinaryMiddleware;
    impl RequestMiddleware for BinaryMiddleware {
        fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
            MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::BadRequest,
                headers: SimpleHeaders::new(),
                body: Some(SendSafeBody::Bytes(vec![0x01, 0x02, 0x03])),
                trailers: SimpleHeaders::new(),
            })
        }
    }

    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(BinaryMiddleware);
    let (_, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    let response = short_circuit.unwrap();
    assert_eq!(response.status, 400);
    assert_eq!(response.body, Some(vec![0x01, 0x02, 0x03]));
}

#[wasm_test]
fn test_middleware_short_circuit_with_headers() {
    struct HeaderMiddleware;
    impl RequestMiddleware for HeaderMiddleware {
        fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
            let mut headers = SimpleHeaders::new();
            headers
                .entry(SimpleHeader::from("WWW-Authenticate".to_string()))
                .or_default()
                .push("Bearer realm=\"test\"".to_string());
            MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::Unauthorized,
                headers,
                body: Some(SendSafeBody::Text("Unauthorized".into())),
                trailers: SimpleHeaders::new(),
            })
        }
    }

    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.middleware(HeaderMiddleware);
    let (_, short_circuit) = run_middleware_writer(&app, ctx(), make_request());
    let response = short_circuit.unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(response.headers.len(), 1);
    // HTTP field names are case-insensitive (RFC 9110 section 5.1), and
    // `SimpleHeader` renders the name in lower case.
    let (name, value) = &response.headers[0];
    assert!(
        name.eq_ignore_ascii_case("WWW-Authenticate"),
        "header name: {name}"
    );
    assert_eq!(value, "Bearer realm=\"test\"");
}
