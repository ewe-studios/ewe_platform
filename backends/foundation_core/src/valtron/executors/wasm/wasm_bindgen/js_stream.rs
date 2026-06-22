//! JS ReadableStream ↔ sync Iterator bridge for valtron on wasm32.
//!
//! WHY: On wasm32, HTTP response bodies arrive as JS `ReadableStream` objects
//! and request bodies may need to be sent as `ReadableStream`s. Valtron's
//! executor model is sync-iterator-driven (poll via `Iterator::next()`),
//! so we need adapters in both directions.
//!
//! WHAT: Two constructs:
//! - `JsReadableStreamIterator` — reads a JS `ReadableStream` and yields
//!   `Vec<u8>` chunks through a sync `Iterator`. Analogous to native
//!   `ThreadedIterFuture` which bridges async→sync via a channel.
//! - `iterator_to_readable_stream` — wraps a sync `Iterator<Item = Vec<u8>>`
//!   into a JS `ReadableStream` for use as a `fetch()` request body.
//!
//! HOW: Both use `ConcurrentQueue` as the sync/async bridge.
//! - Reading: `spawn_local` runs an async loop that awaits `reader.read()`
//!   promises and pushes `Vec<u8>` chunks into the queue. The sync iterator
//!   pops from the queue, returning `JsStreamValue::Waiting` when empty
//!   (valtron's JS yielder schedules a setTimeout and resumes).
//! - Writing: A `pull(controller)` JS closure pops from a queue that a
//!   background `spawn_local` task feeds from the Rust iterator, calling
//!   `controller.enqueue(chunk)` or `controller.close()`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use concurrent_queue::ConcurrentQueue;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Value yielded by `JsReadableStreamIterator`.
///
/// Mirrors `ThreadedValue<T, E>` from the native future executor:
/// - `Chunk(Vec<u8>)` — a batch of bytes from the stream
/// - `Waiting` — stream has more data but none is available yet;
///   the valtron executor yields to the JS event loop and retries
#[derive(Debug)]
pub enum JsStreamValue {
    Chunk(Vec<u8>),
    Waiting,
}

/// Sync iterator over a JS `ReadableStream`.
///
/// WHY: Valtron's executor drives `Iterator::next()` in a poll loop.
/// On native, `ThreadedIterFuture` bridges async work via channels
/// on a background thread. On wasm32 (single-threaded), we use
/// `spawn_local` + `ConcurrentQueue` instead.
///
/// WHAT: Spawns an async task that reads chunks from the
/// `ReadableStreamDefaultReader` and pushes them into a queue.
/// `Iterator::next()` pops from the queue:
/// - `Some(Chunk(bytes))` when data is available
/// - `Some(Waiting)` when the queue is empty but the stream isn't done
/// - `None` when the stream is fully consumed
///
/// HOW: The async reader loop calls `reader.read()` (returns a Promise),
/// awaits it via `JsFuture`, extracts the `Uint8Array` value, and
/// pushes bytes into the `ConcurrentQueue`. Sets `done` flag on
/// stream end or error.
pub struct JsReadableStreamIterator {
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    done: Arc<AtomicBool>,
}

impl JsReadableStreamIterator {
    /// Create a new iterator that reads from the given `ReadableStream`.
    ///
    /// Immediately spawns an async task (via `wasm_bindgen_futures::spawn_local`)
    /// to begin reading chunks into the internal queue.
    #[must_use]
    pub fn new(stream: web_sys::ReadableStream) -> Self {
        let queue = Arc::new(ConcurrentQueue::unbounded());
        let done = Arc::new(AtomicBool::new(false));

        let q = queue.clone();
        let d = done.clone();

        wasm_bindgen_futures::spawn_local(async move {
            let reader = stream
                .get_reader()
                .unchecked_into::<web_sys::ReadableStreamDefaultReader>();

            loop {
                let result = match JsFuture::from(reader.read()).await {
                    Ok(val) => val,
                    Err(_) => {
                        d.store(true, Ordering::Release);
                        break;
                    }
                };

                let done_flag = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                if done_flag {
                    d.store(true, Ordering::Release);
                    break;
                }

                if let Ok(value) = js_sys::Reflect::get(&result, &JsValue::from_str("value")) {
                    let array = js_sys::Uint8Array::new(&value);
                    let bytes = array.to_vec();
                    if !bytes.is_empty() {
                        let _ = q.push(bytes);
                    }
                }
            }
        });

        Self { queue, done }
    }

    /// Whether the underlying stream has been fully consumed and the queue
    /// is empty (no more chunks will ever arrive).
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.done.load(Ordering::Acquire) && self.queue.is_empty()
    }
}

// SAFETY: wasm32 is single-threaded; the !Send JS types live inside the
// spawned async task, not in the iterator itself.
unsafe impl Send for JsReadableStreamIterator {}

impl Iterator for JsReadableStreamIterator {
    type Item = JsStreamValue;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(chunk) => Some(JsStreamValue::Chunk(chunk)),
            Err(_) => {
                if self.done.load(Ordering::Acquire) {
                    None
                } else {
                    Some(JsStreamValue::Waiting)
                }
            }
        }
    }
}

/// Convert a sync byte iterator into a JS `ReadableStream`.
///
/// WHY: The `fetch()` API accepts `ReadableStream` as a request body,
/// enabling streaming uploads. `SendSafeBody` streaming variants
/// (Stream, ChunkedStream, LineFeedStream) hold sync iterators that
/// yield byte chunks — this function bridges them to JS.
///
/// WHAT: Creates a `ReadableStream` with a pull-based underlying source.
/// Each time JS pulls, a chunk is taken from the Rust iterator and
/// enqueued on the controller. When the iterator is exhausted, the
/// controller is closed.
///
/// HOW: Builds a JS object with a `pull(controller)` closure that
/// captures the iterator (behind `Rc<RefCell<..>>` since closures
/// are `FnMut` and wasm is single-threaded). The closure returns a
/// resolved Promise after each enqueue/close.
///
/// # Errors
///
/// Returns `JsValue` if `ReadableStream` construction fails.
pub fn iterator_to_readable_stream(
    iter: Box<dyn Iterator<Item = Vec<u8>>>,
) -> Result<web_sys::ReadableStream, JsValue> {
    use std::cell::RefCell;
    use std::rc::Rc;

    let iter = Rc::new(RefCell::new(iter));

    let pull_iter = iter.clone();
    let pull = Closure::wrap(Box::new(move |controller: JsValue| -> js_sys::Promise {
        let controller: web_sys::ReadableStreamDefaultController =
            controller.unchecked_into();

        let mut iter_ref = pull_iter.borrow_mut();
        match iter_ref.next() {
            Some(bytes) => {
                let array = js_sys::Uint8Array::from(bytes.as_slice());
                let _ = controller.enqueue_with_chunk(&array.into());
            }
            None => {
                let _ = controller.close();
            }
        }

        js_sys::Promise::resolve(&JsValue::UNDEFINED)
    }) as Box<dyn FnMut(JsValue) -> js_sys::Promise>);

    let source = js_sys::Object::new();
    js_sys::Reflect::set(&source, &JsValue::from_str("pull"), pull.as_ref())?;

    // Prevent the closure from being dropped while the stream is alive.
    // On wasm32 single-threaded, the stream and closure share the same
    // event loop lifetime — the closure is invoked synchronously during
    // pull and the ReadableStream prevents GC of the underlying source.
    pull.forget();

    web_sys::ReadableStream::new_with_underlying_source(&source)
}
