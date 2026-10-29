//! Wasm SSE read-error test.
//!
//! WHY: on wasm, SSE bytes come from a JS `ReadableStream` read by
//! `spawn_stream_reader`. A read that rejects must surface as a failure, not a
//! clean end — otherwise a broken feed stalls its consumer instead of failing
//! it (spec 70 feature 07 §3).
//!
//! WHAT: build a `ReadableStream` that yields one `data: hello` event and then
//! errors, feed it to `WasmSseIterator`, and assert the consumer sees
//! `Connecting`, then the event, then `Pending(Failed)`, then the end.
//!
//! HOW: the stream's `pull` callback enqueues the event on its first call and
//! errors the controller on the next. The test polls the iterator and yields to
//! the JS event loop between polls so the reader task can run.

use foundation_core::valtron::Stream;
use foundation_netio::shared::client::http_client::SseProgress;
use foundation_netio::wasm::client::WasmSseIterator;
use foundation_testbed::bindgen::wasm_bindgen::closure::Closure;
use foundation_testbed::bindgen::wasm_bindgen::{JsCast, JsValue};
use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures, web_sys};
use wasm_bindgen_test::wasm_bindgen_test;

async fn yield_now() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .expect("window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
            .expect("set_timeout");
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// A `ReadableStream` that enqueues one SSE event on its first `pull` and
/// errors the controller on the next.
fn stream_that_errors_after_one_event() -> web_sys::ReadableStream {
    use std::cell::Cell;
    use std::rc::Rc;

    let pulls = Rc::new(Cell::new(0u32));

    let pull = {
        let pulls = Rc::clone(&pulls);
        Closure::<dyn FnMut(JsValue)>::new(move |controller: JsValue| {
            let controller: web_sys::ReadableStreamDefaultController = controller.unchecked_into();
            let n = pulls.get();
            pulls.set(n + 1);
            if n == 0 {
                let chunk = js_sys::Uint8Array::from(&b"data: hello\n\n"[..]);
                controller
                    .enqueue_with_chunk(&chunk)
                    .expect("enqueue the event");
            } else {
                controller
                    .error_with_e(&JsValue::from_str("synthetic SSE read error"))
                    .expect("error the stream");
            }
        })
    };

    let underlying = web_sys::UnderlyingSource::new();
    underlying.set_pull(pull.into_js_value().unchecked_ref::<js_sys::Function>());
    web_sys::ReadableStream::new_with_underlying_source(&underlying).expect("stream")
}

#[wasm_bindgen_test]
async fn sse_read_error_surfaces_failed_not_a_clean_end() {
    let mut events = WasmSseIterator::new(stream_that_errors_after_one_event());

    // First poll is the deferred `Connecting` progress.
    match events.next() {
        Some(Stream::Pending(SseProgress::Connecting)) => {}
        other => panic!("expected Pending(Connecting), got {other:?}"),
    }

    // Then the event, then Failed, then the end — yielding so the reader task
    // can push its chunk and record its error.
    let mut saw_event = false;
    let mut saw_failed = false;
    for _ in 0..2000 {
        match events.next() {
            Some(Stream::Pending(SseProgress::Failed(msg))) => {
                assert!(!msg.is_empty(), "the failure carries a message");
                saw_failed = true;
                break;
            }
            Some(Stream::Next(_)) => saw_event = true,
            Some(Stream::Wait) | Some(Stream::Pending(_)) => yield_now().await,
            None => panic!("the stream ended without surfacing the read error"),
        }
    }
    assert!(saw_event, "the consumer must get the event before the failure");
    assert!(saw_failed, "the consumer must see Pending(Failed)");
    assert!(events.next().is_none(), "a failed stream ends");
}
