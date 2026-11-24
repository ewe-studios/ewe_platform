//! The Durable Object stub client against a real local Durable Object.
//!
//! WHY: `StubHttpClient` compiles in a browser build, but only a real stub
//! proves that the namespace lookup, `get_stub()` and `fetch_with_request`
//! carry a request and a streaming response (spec 85 task T5, issue #710).
//! WHAT: one POST through `StubHttpClient` to the `TEST_DO` binding; the
//! Durable Object echoes the request body.
//! HOW: the testbed worker binds `TEST_DO` and sets `globalThis.__CF_TEST_ENV__`
//! (see the `bindgen-wrangler` template).

use foundation_core::url::Uri;
use foundation_core::valtron::Stream;
use foundation_netio::cloudflare::StubHttpClient;
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::client::{HttpClient, PreparedRequest};
use foundation_netio::shared::http::{Extensions, SendSafeBody, SimpleHeaders, SimpleMethod};
use foundation_netio_wasm_cf_tests::test_env;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
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
    let _ = JsFuture::from(promise).await;
}

#[wasm_bindgen_test]
async fn stub_client_sends_and_streams_a_response() {
    let client = StubHttpClient::new(&test_env(), "TEST_DO", "stub-client-test")
        .expect("the TEST_DO binding resolves");

    let sent = b"hello durable object".to_vec();
    let request = PreparedRequest {
        method: SimpleMethod::POST,
        url: Uri::parse("http://do.internal/rpc").expect("uri"),
        headers: SimpleHeaders::new(),
        body: SendSafeBody::Bytes(sent.clone()),
        extensions: Extensions::new(),
        body_readiness: None,
    };

    let response = client.send_async(request).await.expect("stub send");
    let (status, _headers, body) = response.into_parts();
    let status: usize = status.into();
    assert_eq!(status, 200, "the Durable Object answers 200");

    // Read the stream one chunk at a time and yield to the event loop on Wait.
    let mut iter = SendSafeBodyBytesIterator::new(body);
    let mut received = Vec::new();
    let mut ended = false;
    for _ in 0..2000 {
        match iter.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                received.extend_from_slice(&bytes);
            }
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(error))) => {
                panic!("response body failed: {error}")
            }
            Some(Stream::Wait) | Some(Stream::Ignore) => yield_now().await,
            Some(_) => yield_now().await,
            None => {
                ended = true;
                break;
            }
        }
    }
    assert!(ended, "the response body never ended");
    assert_eq!(received, sent, "the response body streams back");
}
