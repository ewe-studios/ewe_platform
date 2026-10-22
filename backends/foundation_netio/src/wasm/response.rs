//! Streaming `Response` bodies for wasm request handlers (Cloudflare Workers,
//! service workers).
//!
//! WHY: a handler that streams (an LLM reply, a log tail) should hand over a
//! Rust `Stream` and get a `web_sys::Response`, without writing
//! `ReadableStream` / controller / closure glue each time.
//!
//! WHAT: [`streaming_response`] (bytes, any content type) and
//! [`sse_response`] (each item becomes one Server-Sent Event).
//!
//! HOW: a **pull**-based `ReadableStream`: the runtime asks for the next chunk
//! only when the client has consumed the previous one, so nothing is buffered
//! ahead of the reader. When the client goes away the runtime cancels the
//! stream and the source is dropped, which stops whatever produces it (e.g.
//! the model call). The JS callbacks are handed to the JS garbage collector
//! (`Closure::into_js_value`), so a response leaks nothing.

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Rc;

use futures_core::Stream;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

type Source = Pin<Box<dyn Stream<Item = Vec<u8>>>>;

/// A `200` response whose body is `body`, pulled one chunk at a time.
///
/// # Errors
///
/// The `JsValue` error the runtime raised while building the response.
pub fn streaming_response<S>(body: S, content_type: &str) -> Result<web_sys::Response, JsValue>
where
    S: Stream<Item = Vec<u8>> + 'static,
{
    let source: Rc<RefCell<Option<Source>>> = Rc::new(RefCell::new(Some(Box::pin(body))));
    let cancelled = Rc::new(Cell::new(false));

    let pull = {
        let source = Rc::clone(&source);
        let cancelled = Rc::clone(&cancelled);
        Closure::<dyn FnMut(JsValue) -> js_sys::Promise>::new(move |controller: JsValue| {
            let controller: web_sys::ReadableStreamDefaultController = controller.unchecked_into();
            let source = Rc::clone(&source);
            let cancelled = Rc::clone(&cancelled);
            // `pull` is not called again until this promise settles, so the
            // source is taken out for the await and put back afterwards.
            wasm_bindgen_futures::future_to_promise(async move {
                let Some(mut body) = source.borrow_mut().take() else {
                    return Ok(JsValue::UNDEFINED);
                };
                match std::future::poll_fn(|cx| body.as_mut().poll_next(cx)).await {
                    Some(chunk) if !cancelled.get() => {
                        controller.enqueue_with_chunk(&js_sys::Uint8Array::from(chunk.as_slice()))?;
                        *source.borrow_mut() = Some(body);
                    }
                    Some(_) => {}
                    None => controller.close()?,
                }
                Ok(JsValue::UNDEFINED)
            })
        })
    };
    let cancel = Closure::<dyn FnMut(JsValue)>::new(move |_reason: JsValue| {
        cancelled.set(true);
        source.borrow_mut().take();
    });

    let underlying = web_sys::UnderlyingSource::new();
    underlying.set_pull(pull.into_js_value().unchecked_ref::<js_sys::Function>());
    underlying.set_cancel(cancel.into_js_value().unchecked_ref::<js_sys::Function>());
    let stream = web_sys::ReadableStream::new_with_underlying_source(&underlying)?;

    let headers = web_sys::Headers::new()?;
    headers.set("Content-Type", content_type)?;
    headers.set("Cache-Control", "no-cache")?;
    let init = web_sys::ResponseInit::new();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_readable_stream_and_init(Some(&stream), &init)
}

/// A `text/event-stream` response with one event per item of `events`. An
/// item with newlines becomes one `data:` line per line, which `EventSource`
/// (and any SSE parser) joins back with `\n`.
///
/// # Errors
///
/// As [`streaming_response`].
pub fn sse_response<S>(events: S) -> Result<web_sys::Response, JsValue>
where
    S: Stream<Item = String> + 'static,
{
    streaming_response(SseFrames(Box::pin(events)), "text/event-stream; charset=utf-8")
}

/// The SSE wire form of one event's data.
#[must_use]
pub fn sse_frame(data: &str) -> String {
    let mut frame = String::with_capacity(data.len() + 8);
    for line in data.split('\n') {
        frame.push_str("data: ");
        frame.push_str(line.strip_suffix('\r').unwrap_or(line));
        frame.push('\n');
    }
    frame.push('\n');
    frame
}

struct SseFrames(Pin<Box<dyn Stream<Item = String>>>);

impl Stream for SseFrames {
    type Item = Vec<u8>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Vec<u8>>> {
        self.0
            .as_mut()
            .poll_next(cx)
            .map(|event| event.map(|data| sse_frame(&data).into_bytes()))
    }
}
