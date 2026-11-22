//! Wasm response body tests: streaming, the read-ahead bound, the body limit
//! and cancellation.
//!
//! WHY: the wasm fetch client must hand a response body to its consumer one
//! chunk at a time (root rule 4), honour `max_body_size`, never queue more than
//! `body_queue_chunks` chunks, and stop the transfer when the consumer goes
//! away (rules 1, 5, 7 and 10 of spec 85).
//!
//! WHAT: `WasmByteStream` over synthetic JS streams for the bound and cancel
//! cases; `FetchHttpClient` against the testbed's streaming endpoint for the
//! fetch cases.
//!
//! HOW: the Deno runner starts an echo server and sets
//! `globalThis.__FWT_BASE_URL__`. `/stream` serves a streaming body of
//! `chunks` chunks of `size` bytes.

use std::cell::Cell;
use std::rc::Rc;

use foundation_core::url::Uri;
use foundation_core::valtron::Stream;
use foundation_netio::shared::client::body_reader::{SendSafeBodyBytesItem, SendSafeBodyBytesIterator};
use foundation_netio::shared::client::{ClientConfig, HttpClient, PreparedRequest};
use foundation_netio::shared::http::{Extensions, SendSafeBody, SimpleHeaders, SimpleMethod};
use foundation_netio::wasm::client::stream::WasmByteStream;
use foundation_netio::wasm::client::FetchHttpClient;
use foundation_testbed::bindgen::wasm_bindgen::closure::Closure;
use foundation_testbed::bindgen::wasm_bindgen::{JsCast, JsValue};
use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures, web_sys};
use wasm_bindgen_test::wasm_bindgen_test;

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
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// The base URL of the testbed echo server.
fn base_url() -> String {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__FWT_BASE_URL__"))
        .expect("the Deno runner sets globalThis.__FWT_BASE_URL__")
        .as_string()
        .expect("__FWT_BASE_URL__ is a string")
}

fn get_request(url: String) -> PreparedRequest {
    PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse(&url).expect("uri"),
        headers: SimpleHeaders::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// A JS `ReadableStream` that enqueues one chunk on each pull, up to `total`,
/// and records whether the consumer cancelled it.
fn counting_stream(total: u32, size: usize, cancelled: Rc<Cell<bool>>) -> web_sys::ReadableStream {
    let sent = Rc::new(Cell::new(0u32));

    let pull = {
        let sent = Rc::clone(&sent);
        Closure::<dyn FnMut(JsValue)>::new(move |controller: JsValue| {
            let controller: web_sys::ReadableStreamDefaultController = controller.unchecked_into();
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

    let cancel = {
        let cancelled = Rc::clone(&cancelled);
        Closure::<dyn FnMut(JsValue)>::new(move |_reason: JsValue| {
            cancelled.set(true);
        })
    };

    let underlying = web_sys::UnderlyingSource::new();
    underlying.set_pull(pull.into_js_value().unchecked_ref::<js_sys::Function>());
    underlying.set_cancel(cancel.into_js_value().unchecked_ref::<js_sys::Function>());
    web_sys::ReadableStream::new_with_underlying_source(&underlying).expect("stream")
}

/// Drive `send_async` against `/stream` and count the chunks the consumer gets.
async fn read_stream_body(url: String, config: ClientConfig) -> (usize, usize) {
    let client = FetchHttpClient::with_config(config);
    let response = client.send_async(get_request(url)).await.expect("send_async");
    let (status, _headers, body) = response.into_parts();
    let status: usize = status.into();
    assert_eq!(status, 200, "the stream endpoint answers 200");
    assert!(
        matches!(&body, SendSafeBody::Stream(Some(_))),
        "send_async must return a streaming body, not buffered text"
    );

    let mut iter = SendSafeBodyBytesIterator::new(body);
    let mut chunks = 0usize;
    let mut bytes = 0usize;
    loop {
        match iter.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(chunk))) => {
                chunks += 1;
                bytes += chunk.len();
            }
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(error))) => {
                panic!("body stream failed: {error}")
            }
            Some(Stream::Wait) | Some(Stream::Ignore) => yield_now().await,
            Some(_) => yield_now().await,
            None => break,
        }
    }
    (chunks, bytes)
}

#[wasm_bindgen_test]
async fn send_async_streams_body() {
    // 50 chunks of 8 bytes: the client must receive every chunk.
    let (chunks, bytes) = read_stream_body(
        format!("{}/stream?chunks=50&size=8", base_url()),
        ClientConfig::default().with_body_queue_chunks(4),
    )
    .await;

    assert_eq!(bytes, 400, "every body byte arrives");
    assert!(chunks >= 1, "the body arrives in at least one chunk");
}

#[wasm_bindgen_test]
async fn body_limit() {
    let client = FetchHttpClient::with_config(
        ClientConfig::default()
            .with_max_body_size(Some(1024))
            .with_body_queue_chunks(4),
    );
    let response = client
        .send_async(get_request(format!(
            "{}/stream?chunks=2048&size=1",
            base_url()
        )))
        .await
        .expect("send_async");
    let (_status, _headers, body) = response.into_parts();

    let mut iter = SendSafeBodyBytesIterator::new(body);
    let mut bytes = 0usize;
    let mut failure = None;
    loop {
        match iter.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(chunk))) => bytes += chunk.len(),
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(error))) => {
                failure = Some(error.to_string());
                break;
            }
            Some(Stream::Wait) | Some(Stream::Ignore) => yield_now().await,
            Some(_) => yield_now().await,
            None => break,
        }
    }

    let message = failure.expect("the body must fail at the limit");
    assert!(
        message.contains("max_body_size"),
        "the error names the limit: {message}"
    );
    assert!(bytes <= 1024, "no more than the limit is handed out: {bytes}");
}

#[wasm_bindgen_test]
async fn queue_is_bounded() {
    let cancelled = Rc::new(Cell::new(false));
    let stream = counting_stream(100, 8, Rc::clone(&cancelled));
    let body = WasmByteStream::new(stream, 2, None);

    // Let the reader task fill the queue.
    for _ in 0..20 {
        yield_now().await;
    }
    assert!(
        body.buffered_chunks() <= 2,
        "the read-ahead stays at body_queue_chunks: {}",
        body.buffered_chunks()
    );
}

#[wasm_bindgen_test]
async fn cancel_drops_source() {
    let cancelled = Rc::new(Cell::new(false));
    let stream = counting_stream(1000, 8, Rc::clone(&cancelled));

    {
        let body = WasmByteStream::new(stream, 2, None);
        // Drop after the reader has started.
        for _ in 0..5 {
            yield_now().await;
        }
        drop(body);
    }

    for _ in 0..200 {
        if cancelled.get() {
            return;
        }
        yield_now().await;
    }
    panic!("the source never saw the cancel");
}
