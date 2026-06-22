//! Wasm SSE stream adapter.
//!
//! WHY: On wasm32, SSE responses arrive as JS `ReadableStream` objects.
//! The AI providers consume SSE through valtron's sync `Iterator<Item =
//! Stream<ParseResult, SseProgress>>` interface. This module bridges
//! the two using valtron's `JsReadableStreamIterator`.
//!
//! WHAT: `WasmSseIterator` reads byte chunks from a JS ReadableStream,
//! feeds them through the shared `SseParser`, and yields parsed SSE events.
//!
//! HOW: Valtron's `JsReadableStreamIterator` spawns an async task that
//! reads the JS stream into a `ConcurrentQueue`. A `QueueReader` adapts
//! the queue into `std::io::Read` for `SseParser<R: Read>`. The iterator
//! returns `Stream::Wait` when no data is available yet (valtron yields
//! to the JS event loop), `Stream::Next` for parsed events, and `None`
//! when the stream is fully consumed.

use std::io::{Cursor, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::Stream;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::event_source::shared::sse::SseParser;
use crate::event_source::ParseResult;
use crate::simple_http::client::shared::http_client::SseProgress;
use foundation_core::io::ioutils::SharedByteBufferStream;

/// `Read` adapter over a `ConcurrentQueue<Vec<u8>>` with shared done flag.
///
/// WHY: `SseParser` requires `std::io::Read`. Byte chunks arrive
/// asynchronously from the JS ReadableStream reader task.
///
/// WHAT: Buffers one chunk at a time. Returns `Ok(0)` when the queue
/// is temporarily empty (parser retries on next poll) or when the
/// stream is truly exhausted (EOF).
///
/// HOW: Holds `Arc<ConcurrentQueue>` shared with the async reader task.
/// The cursor tracks position within the current chunk.
struct QueueReader {
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    current: Cursor<Vec<u8>>,
}

impl QueueReader {
    fn new(queue: Arc<ConcurrentQueue<Vec<u8>>>) -> Self {
        Self {
            queue,
            current: Cursor::new(Vec::new()),
        }
    }
}

impl Read for QueueReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.current.get_ref().len() - self.current.position() as usize;
        if remaining > 0 {
            return self.current.read(buf);
        }

        match self.queue.pop() {
            Ok(chunk) => {
                self.current = Cursor::new(chunk);
                self.current.read(buf)
            }
            Err(_) => Ok(0),
        }
    }
}

/// Spawn an async task that reads all chunks from a JS `ReadableStream`
/// into the shared queue.
///
/// Each chunk is a `Uint8Array` pushed as `Vec<u8>`. When the stream
/// ends or errors, `done` is set to `true`.
fn spawn_stream_reader(
    body: web_sys::ReadableStream,
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    done: Arc<AtomicBool>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let reader = body
            .get_reader()
            .unchecked_into::<web_sys::ReadableStreamDefaultReader>();

        loop {
            let result = match JsFuture::from(reader.read()).await {
                Ok(val) => val,
                Err(e) => {
                    tracing::error!("SSE ReadableStream read error: {e:?}");
                    done.store(true, Ordering::Release);
                    break;
                }
            };

            let done_flag = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            if done_flag {
                done.store(true, Ordering::Release);
                break;
            }

            if let Ok(value) = js_sys::Reflect::get(&result, &JsValue::from_str("value")) {
                let array = js_sys::Uint8Array::new(&value);
                let bytes = array.to_vec();
                if !bytes.is_empty() {
                    if queue.push(bytes).is_err() {
                        done.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        }
    });
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
/// HOW: On construction, spawns an async reader task (via
/// `wasm_bindgen_futures::spawn_local`) that reads JS stream chunks
/// into a `ConcurrentQueue`. The `SseParser` reads from `QueueReader`
/// which pops chunks from the same queue. A separate `Arc<AtomicBool>`
/// clone tracks whether the JS stream is fully consumed.
pub struct WasmSseIterator {
    parser: SseParser<QueueReader>,
    done: Arc<AtomicBool>,
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    started: bool,
}

impl WasmSseIterator {
    pub fn new(body: web_sys::ReadableStream) -> Self {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        let done = Arc::new(AtomicBool::new(false));

        spawn_stream_reader(body, queue.clone(), done.clone());

        let reader = QueueReader::new(queue.clone());
        let buffer = SharedByteBufferStream::ref_cell(reader);
        let parser = SseParser::new(buffer);

        Self {
            parser,
            done,
            queue,
            started: false,
        }
    }
}

// SAFETY: wasm32 is single-threaded; the !Send JS types live inside the
// spawned async task, not in the iterator struct.
unsafe impl Send for WasmSseIterator {}

impl Iterator for WasmSseIterator {
    type Item = Stream<ParseResult, SseProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            return Some(Stream::Pending(SseProgress::Connecting));
        }

        match self.parser.parse_next() {
            Ok(Some(result)) => Some(Stream::Next(result)),
            Ok(None) | Err(_) => {
                if self.done.load(Ordering::Acquire) && self.queue.is_empty() {
                    None
                } else {
                    Some(Stream::Wait)
                }
            }
        }
    }
}
