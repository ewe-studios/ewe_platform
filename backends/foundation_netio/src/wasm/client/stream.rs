//! Wasm response byte streams.
//!
//! WHY: On wasm32 an HTTP response body is a JS `ReadableStream`. The ordinary
//! `send_async` path and the server-sent event (SSE) path both need its bytes,
//! and neither may hold the whole body in memory (root rule 4).
//!
//! WHAT: [`WasmByteStream`] reads a JS `ReadableStream` one chunk at a time and
//! yields `Data::Bytes`, `Data::Retry` and errors. [`WasmSseIterator`] parses
//! the same reader into SSE events.
//!
//! HOW: One spawned task owns the `ReadableStreamDefaultReader` and pushes each
//! `Uint8Array` into a `ConcurrentQueue`. The queue is bounded by a chunk count.
//! A `WakeRegistry` fires on each push and at the end, so a valtron stream that
//! parks is polled again as soon as data arrives. The reader counts the bytes it
//! hands out and stops at the configured body limit. Dropping the stream sets a
//! cancel flag; the task calls `reader.cancel()` and closes the queue.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use concurrent_queue::ConcurrentQueue;
use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use foundation_core::valtron::{Stream, WakeCallback, WakeRegistry, WakeResult, WakeableStream};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::event_source::ParseResult;
use crate::shared::client::http_client::SseProgress;
use crate::shared::client::queue_reader::{QueueReader, QueueSseEvents};
use crate::shared::http::HttpClientError;

/// The shared state of one response stream.
///
/// All fields are `Arc` handles to `Send + Sync` state. The JS reader lives in
/// the spawned task, so this type is `Send + Sync` with no `unsafe impl` and no
/// `SendWrapper`.
struct ReaderParts {
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    done: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    cancelled: Arc<AtomicBool>,
}

/// The bytes of one wasm response, read one chunk at a time.
///
/// The JS reader lives inside the spawned task, so this type holds only `Arc`
/// state. It is `Send + Sync` with no `unsafe impl` and no `SendWrapper`.
pub struct WasmByteStream {
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    done: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    cancelled: Arc<AtomicBool>,
}

impl WasmByteStream {
    /// Read `stream`, with at most `chunks` queued ahead of the consumer, and
    /// stop after `max` bytes.
    ///
    /// `chunks` is the configured read-ahead bound (`body_queue_chunks`). A
    /// browser chunk is 16 KiB to 64 KiB, so 16 chunks hold 256 KiB to 1 MiB.
    /// `max` is the configured body limit; `None` means no limit.
    #[must_use]
    pub fn new(stream: web_sys::ReadableStream, chunks: usize, max: Option<usize>) -> Self {
        let parts = start_reader(stream, chunks, max, Arc::new(WakeRegistry::new()));
        Self {
            queue: parts.queue,
            done: parts.done,
            error: parts.error,
            cancelled: parts.cancelled,
        }
    }

    /// The same bytes as a `std::io::Read`, for the server-sent event parser.
    #[must_use]
    pub fn reader(&self) -> QueueReader {
        QueueReader::new(self.queue.clone(), self.done.clone())
    }

    /// The number of chunks waiting for the consumer.
    ///
    /// This is a test hook: a test proves that the read-ahead stays at or below
    /// `body_queue_chunks`. It is not part of the stable API.
    #[doc(hidden)]
    #[must_use]
    pub fn buffered_chunks(&self) -> usize {
        self.queue.len()
    }
}

impl Iterator for WasmByteStream {
    type Item = Result<Data, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(bytes) => Some(Ok(Data::Bytes(bytes))),
            Err(_) => {
                // A read error or an over-limit body that the reader task
                // recorded. It surfaces once the queued chunks are drained.
                if let Ok(mut slot) = self.error.lock() {
                    if let Some(message) = slot.take() {
                        return Some(Err(Box::new(HttpClientError::Reason(message))));
                    }
                }
                if self.done.load(Ordering::Acquire) {
                    None
                } else {
                    Some(Ok(Data::Retry))
                }
            }
        }
    }
}

impl Drop for WasmByteStream {
    fn drop(&mut self) {
        // A dropped response stops the transfer: the reader task calls
        // `reader.cancel()` and closes the queue on its next turn.
        self.cancelled.store(true, Ordering::Release);
        self.queue.close();
    }
}

/// Create the shared state, spawn the reader task and return the handles.
fn start_reader(
    stream: web_sys::ReadableStream,
    chunks: usize,
    max: Option<usize>,
    wake: Arc<WakeRegistry>,
) -> ReaderParts {
    let queue = Arc::new(ConcurrentQueue::unbounded());
    let done = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None));
    let cancelled = Arc::new(AtomicBool::new(false));

    spawn_stream_reader(
        stream,
        queue.clone(),
        done.clone(),
        error.clone(),
        cancelled.clone(),
        wake.clone(),
        chunks.max(1),
        max,
    );

    ReaderParts {
        queue,
        done,
        error,
        cancelled,
    }
}

/// Spawn an async task that reads all chunks from a JS `ReadableStream` into
/// the shared queue.
///
/// Each chunk is a `Uint8Array` pushed as `Vec<u8>`. The task stops after
/// `chunks` queued chunks until the consumer takes one, so a slow consumer does
/// not grow memory. It stops at `max` bytes and records an error that names the
/// limit. When the stream ends, `done` is set. A JS read error is recorded in
/// `error` as well, so the consumer surfaces a failure instead of a clean end.
/// Every readiness edge (a push or a transition to closed) fires `wake`.
fn spawn_stream_reader(
    body: web_sys::ReadableStream,
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    done: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    cancelled: Arc<AtomicBool>,
    wake: Arc<WakeRegistry>,
    chunks: usize,
    max: Option<usize>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let reader = body
            .get_reader()
            .unchecked_into::<web_sys::ReadableStreamDefaultReader>();

        let mut total = 0usize;
        loop {
            if cancelled.load(Ordering::Acquire) {
                let _ = JsFuture::from(reader.cancel()).await;
                break;
            }

            // Wait for room before reading the next chunk, so at most `chunks`
            // chunks are held ahead of the consumer.
            while queue.len() >= chunks && !cancelled.load(Ordering::Acquire) {
                yield_to_event_loop().await;
            }
            if cancelled.load(Ordering::Acquire) {
                let _ = JsFuture::from(reader.cancel()).await;
                break;
            }

            let result = match JsFuture::from(reader.read()).await {
                Ok(val) => val,
                Err(e) => {
                    set_error(&error, format!("ReadableStream read error: {e:?}"));
                    break;
                }
            };

            let done_flag = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            if done_flag {
                break;
            }

            if let Ok(value) = js_sys::Reflect::get(&result, &JsValue::from_str("value")) {
                let array = js_sys::Uint8Array::new(&value);
                let bytes = array.to_vec();
                if bytes.is_empty() {
                    continue;
                }
                if let Some(limit) = max {
                    // The limit counts the bytes handed to the caller. A chunk
                    // that would pass it is refused, so the caller never sees
                    // more than `limit` bytes.
                    if total.saturating_add(bytes.len()) > limit {
                        set_error(
                            &error,
                            format!("response body exceeds max_body_size of {limit} bytes"),
                        );
                        // Release the connection: no more bytes are wanted.
                        let _ = JsFuture::from(reader.cancel()).await;
                        break;
                    }
                }
                total = total.saturating_add(bytes.len());
                tracing::trace!("response chunk: {} bytes", bytes.len());
                if queue.push(bytes).is_err() {
                    break;
                }
                wake.fire();
            }
        }

        done.store(true, Ordering::Release);
        queue.close();
        wake.fire();
    });
}

fn set_error(slot: &Arc<Mutex<Option<String>>>, message: String) {
    tracing::error!("{message}");
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(message);
    }
}

/// Yield to the JavaScript event loop, so the reader task and the consumer make
/// progress on the single wasm thread.
///
/// The macrotask queue is the right level: a microtask yield alone can starve
/// the timer and network callbacks that produce the next chunk. `setTimeout`
/// exists in a browser, a Worker and Deno.
async fn yield_to_event_loop() {
    let global = js_sys::global();
    let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
        .ok()
        .and_then(|value| value.dyn_ref::<js_sys::Function>().cloned());
    if let Some(set_timeout) = set_timeout {
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let _ = set_timeout.call2(&global, &resolve, &JsValue::from_f64(0.0));
        });
        let _ = JsFuture::from(promise).await;
        return;
    }
    // No timer: a microtask yield is the only option left.
    let promise = js_sys::Promise::resolve(&JsValue::UNDEFINED);
    let _ = JsFuture::from(promise).await;
}

/// Iterator that yields `Stream<ParseResult, SseProgress>` from a wasm
/// `ReadableStream`.
///
/// WHY: AI providers expect SSE events as `Iterator<Item = Stream<
/// ParseResult, SseProgress>>`. On wasm the raw bytes come from a JS
/// ReadableStream.
///
/// WHAT: Wraps a shared queue → `QueueReader` → `SseParser` pipeline.
/// Maps parser output into valtron `Stream` variants.
///
/// HOW: Shares the [`WasmByteStream`] reader task, so the SSE path has the same
/// chunk bound, body limit and cancel behaviour as the ordinary body path.
pub struct WasmSseIterator {
    events: QueueSseEvents,
    cancelled: Arc<AtomicBool>,
    wake: Arc<WakeRegistry>,
    started: bool,
}

impl WasmSseIterator {
    /// Events from `body`, with the default client bounds.
    #[must_use]
    pub fn new(body: web_sys::ReadableStream) -> Self {
        let config = crate::shared::client::ClientConfig::default();
        Self::new_with_limits(body, config.body_queue_chunks, config.max_body_size)
    }

    /// Events from `body`, with the given read-ahead bound and byte limit.
    #[must_use]
    pub fn new_with_limits(
        body: web_sys::ReadableStream,
        chunks: usize,
        max_body_size: Option<u64>,
    ) -> Self {
        let wake = Arc::new(WakeRegistry::new());
        let parts = start_reader(body, chunks, usize_limit(max_body_size), wake.clone());

        Self {
            events: QueueSseEvents::new_with_error(
                parts.queue,
                parts.done,
                parts.error,
                wake.clone(),
            ),
            cancelled: parts.cancelled,
            wake,
            started: false,
        }
    }
}

// SAFETY: wasm32 is single-threaded; the !Send JS types live inside the
// spawned async task, not in the iterator struct. The `WakeRegistry` holds
// non-Send one-shot callbacks by design (valtron's single-threaded wake path).
unsafe impl Send for WasmSseIterator {}

impl Drop for WasmSseIterator {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.wake.fire();
    }
}

impl WakeableStream for WasmSseIterator {
    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        self.events.wake(cb)
    }
}

impl Iterator for WasmSseIterator {
    type Item = Stream<ParseResult, SseProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            return Some(Stream::Pending(SseProgress::Connecting));
        }
        // Parsing, parking and failing (a malformed feed ends the stream with
        // `Pending(Failed)` then `None`) live in `QueueSseEvents`.
        self.events.next()
    }
}

/// Convert the configured `u64` limit to the `usize` the reader counts in.
pub(crate) fn usize_limit(max: Option<u64>) -> Option<usize> {
    max.map(|value| usize::try_from(value).unwrap_or(usize::MAX))
}
