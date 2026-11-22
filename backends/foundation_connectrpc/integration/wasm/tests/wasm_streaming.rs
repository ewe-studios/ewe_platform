//! Wasm streaming tests for foundation_connectrpc and the foundation_http
//! bridges.
//!
//! WHY: the wasm request bridge must stream a request body, the connectrpc sink
//! must stream a response with a real status, and fetch must report the
//! capability it has. These paths cannot build in the packages' own test
//! targets (issue #576), so this standalone crate runs them in the wasm testbed.
//!
//! WHAT: `inbound_body_streams` (spec 85 S5), `error_status_streams` (S6),
//! `cancel_drops_source` (S7) and `capability_constants`.
//!
//! HOW: `request_from_cf` over a synthetic JS stream for the inbound case; the
//! `ConnectSink` for the response cases; the exported constant for the
//! capability case.

use std::cell::Cell;
use std::rc::Rc;

use bytes::Bytes;
use foundation_connectrpc::shared::transport::wasm::WASM_FETCH_CAPABILITIES;
use foundation_connectrpc::{ConnectSink, ResponseSink};
use foundation_core::valtron::Stream;
use foundation_http::wasm::bridge::cf::request_from_cf;
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleOutgoingResponse, Status,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::{closure::Closure, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;
use web_sys::{
    ReadableStream, ReadableStreamDefaultController, ReadableStreamDefaultReader,
    RequestInit, Response, UnderlyingSource,
};

async fn yield_now() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("setTimeout")
            .dyn_into::<js_sys::Function>()
            .expect("setTimeout is a function");
        set_timeout
            .call2(&global, &resolve, &JsValue::from_f64(0.0))
            .expect("setTimeout");
    });
    let _ = JsFuture::from(promise).await;
}

/// A JS `ReadableStream` that enqueues one chunk on each pull, up to `total`,
/// and counts its pulls.
fn counting_stream(total: u32, size: usize) -> (ReadableStream, Rc<Cell<u32>>) {
    let pulls = Rc::new(Cell::new(0u32));
    let sent = Rc::new(Cell::new(0u32));

    let pull = {
        let pulls = Rc::clone(&pulls);
        let sent = Rc::clone(&sent);
        Closure::<dyn FnMut(JsValue)>::new(move |controller: JsValue| {
            let controller: ReadableStreamDefaultController = controller.unchecked_into();
            pulls.set(pulls.get() + 1);
            let n = sent.get();
            if n >= total {
                let _ = controller.close();
                return;
            }
            sent.set(n + 1);
            let chunk = js_sys::Uint8Array::new_with_length(u32::try_from(size).unwrap_or(1));
            chunk.fill(7, 0, u32::try_from(size).unwrap_or(1));
            let _ = controller.enqueue_with_chunk(&chunk);
        })
    };
    let cancel = Closure::<dyn FnMut(JsValue)>::new(|_reason: JsValue| {});

    let underlying = UnderlyingSource::new();
    underlying.set_pull(pull.into_js_value().unchecked_ref::<js_sys::Function>());
    underlying.set_cancel(cancel.into_js_value().unchecked_ref::<js_sys::Function>());
    (
        ReadableStream::new_with_underlying_source(&underlying).expect("stream"),
        pulls,
    )
}

#[wasm_bindgen_test]
async fn inbound_body_streams() {
    // A 1000-chunk request body. Reading one chunk must not read the rest.
    let (stream, pulls) = counting_stream(1000, 16);
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from(stream));
    let request =
        web_sys::Request::new_with_str_and_init("http://example.com/rpc", &init).expect("request");

    let incoming = request_from_cf(&request, 8 * 1024 * 1024).expect("request_from_cf");
    let body = incoming.body.expect("the body is a stream");
    assert!(
        matches!(body, SendSafeBody::Stream(_)),
        "the inbound body must be a stream, not buffered text"
    );

    let mut iter = SendSafeBodyBytesIterator::new(body);
    let mut first = None;
    for _ in 0..200 {
        match iter.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(chunk))) => {
                first = Some(chunk);
                break;
            }
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(error))) => {
                panic!("body stream failed: {error}")
            }
            Some(_) => yield_now().await,
            None => break,
        }
    }
    assert!(first.is_some(), "the handler reads one chunk");
    // The reader may fill its bounded queue (default 16), but it must not read
    // the whole 1000-chunk body for one chunk.
    assert!(
        pulls.get() < 100,
        "one chunk must not read the whole body: {} pulls",
        pulls.get()
    );
}

fn head(status: Status) -> SimpleOutgoingResponse {
    SimpleOutgoingResponse::builder()
        .with_proto(Proto::HTTP11)
        .with_status(status)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/connect+proto")
        .build()
        .expect("valid response head")
}

async fn read_one_chunk(response: &Response) -> Vec<u8> {
    let stream = response.body().expect("a streaming body");
    let reader: ReadableStreamDefaultReader =
        stream.get_reader().unchecked_into::<ReadableStreamDefaultReader>();
    let result = JsFuture::from(reader.read()).await.expect("read");
    let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).expect("value");
    js_sys::Uint8Array::new(&value).to_vec()
}

#[wasm_bindgen_test]
async fn error_status_streams() {
    let (mut sink, handle) = ConnectSink::new_pair();
    sink.open(head(Status::ServiceUnavailable))
        .await
        .expect("open");
    sink.chunk(Bytes::from_static(b"frame-one")).await.expect("chunk");
    sink.finish().await.expect("finish");

    let response = handle.into_response().expect("a response");
    assert_eq!(response.status(), 503, "the sink keeps the error status");
    assert_eq!(read_one_chunk(&response).await, b"frame-one");
}

#[wasm_bindgen_test]
async fn whole_response_keeps_status() {
    let (mut sink, handle) = ConnectSink::new_pair();
    let response = SimpleOutgoingResponse::builder()
        .with_proto(Proto::HTTP11)
        .with_status(Status::ServiceUnavailable)
        .with_body_bytes(b"unavailable".to_vec())
        .build()
        .expect("response");
    sink.whole(response).await.expect("whole");

    let response = handle.into_response().expect("a response");
    assert_eq!(response.status(), 503);
    assert_eq!(read_one_chunk(&response).await, b"unavailable");
}

#[wasm_bindgen_test]
async fn cancel_drops_source() {
    let (mut sink, handle) = ConnectSink::new_pair();
    sink.open(head(Status::OK)).await.expect("open");
    sink.chunk(Bytes::from_static(b"first")).await.expect("chunk");

    // The client goes away: cancelling the response body drops the source.
    let response = handle.into_response().expect("a response");
    let stream = response.body().expect("a streaming body");
    let reader: ReadableStreamDefaultReader =
        stream.get_reader().unchecked_into::<ReadableStreamDefaultReader>();
    JsFuture::from(reader.cancel()).await.expect("cancel");

    // The next frame cannot be delivered: the source is gone.
    let result = sink.chunk(Bytes::from_static(b"second")).await;
    assert!(
        result.is_err(),
        "a cancelled response must end the call, not accept a frame"
    );
}

#[wasm_bindgen_test]
fn capability_constants() {
    assert!(
        WASM_FETCH_CAPABILITIES.request_streaming,
        "fetch streams a request body"
    );
    assert!(!WASM_FETCH_CAPABILITIES.full_duplex);
    assert!(!WASM_FETCH_CAPABILITIES.h2_trailers);
    assert!(!WASM_FETCH_CAPABILITIES.multiplexed);
}
