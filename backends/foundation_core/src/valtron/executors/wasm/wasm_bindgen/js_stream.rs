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
//! - `iterator_to_readable_stream` — wraps a byte source (`JsStreamItem`) in a
//!   JS `ReadableStream` for use as a `fetch()` request body. A `Pending` item
//!   parks the pull on readiness (or a timer) instead of spinning.
//!
//! HOW: Both use `ConcurrentQueue` as the sync/async bridge.
//! - Reading: `spawn_local` runs an async loop that awaits `reader.read()`
//!   promises and pushes `JsStreamValue` items into the queue. The sync
//!   iterator pops from the queue, returning `JsStreamValue::Waiting` when
//!   empty (valtron's JS yielder schedules a setTimeout and resumes).
//!   Errors are pushed as `JsStreamValue::Error` so they flow through the
//!   same channel — no separate error storage needed.
//! - Writing: a `pull(controller)` JS closure asks the source for one item.
//!   `Chunk` enqueues, `Error` fails the stream, the end closes it, and
//!   `Pending` returns a promise that readiness (or `setTimeout(0)`) resolves.
//!   The pull never calls the source again in a loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
/// - `Error(String)` — the stream errored (closed unexpectedly,
///   read failure, etc.). The iterator will return `None` after this.
#[derive(Debug)]
pub enum JsStreamValue {
    Chunk(Vec<u8>),
    Waiting,
    Error(String),
}

/// Sync iterator over a JS `ReadableStream`.
///
/// WHY: Valtron's executor drives `Iterator::next()` in a poll loop.
/// On native, `ThreadedIterFuture` bridges async work via channels
/// on a background thread. On wasm32 (single-threaded), we use
/// `spawn_local` + `ConcurrentQueue` instead.
///
/// WHAT: Spawns an async task that reads chunks from the
/// `ReadableStreamDefaultReader` and pushes them as `JsStreamValue`
/// items into a queue. Errors are pushed as `JsStreamValue::Error`
/// through the same queue — the consumer drains all queued chunks
/// before seeing the error.
///
/// `Iterator::next()` pops from the queue:
/// - `Some(Chunk(bytes))` when data is available
/// - `Some(Error(msg))` when the stream errored
/// - `Some(Waiting)` when the queue is empty but the stream isn't done
/// - `None` when the stream is fully consumed
///
/// HOW: The async reader loop calls `reader.read()` (returns a Promise),
/// awaits it via `JsFuture`, extracts the `Uint8Array` value, and
/// pushes `JsStreamValue::Chunk` into the `ConcurrentQueue`. On error
/// it pushes `JsStreamValue::Error`. If the queue is full, the rejected
/// item is held locally and retried before the next JS read. If the
/// queue is closed (consumer gone), the reader task stops.
pub struct JsReadableStreamIterator {
    queue: Arc<ConcurrentQueue<JsStreamValue>>,
    done: Arc<AtomicBool>,
}

impl JsReadableStreamIterator {
    /// Create a new iterator that reads from the given `ReadableStream`.
    ///
    /// Immediately spawns an async task (via `wasm_bindgen_futures::spawn_local`)
    /// to begin reading chunks into the internal queue.
    /// Bounded queue (default 64 slots) — applies backpressure to the JS
    /// reader when the consumer can't keep up, preventing OOM.
    #[must_use]
    pub fn new(stream: web_sys::ReadableStream) -> Self {
        Self::with_capacity(stream, 512)
    }

    /// Unbounded queue — no backpressure, use only when you know the
    /// stream is short-lived or the consumer drains faster than production.
    #[must_use]
    pub fn unbounded(stream: web_sys::ReadableStream) -> Self {
        Self::create(stream, ConcurrentQueue::unbounded())
    }

    /// Bounded queue with a caller-chosen capacity.
    #[must_use]
    pub fn with_capacity(stream: web_sys::ReadableStream, capacity: usize) -> Self {
        Self::create(stream, ConcurrentQueue::bounded(capacity))
    }

    fn create(stream: web_sys::ReadableStream, queue: ConcurrentQueue<JsStreamValue>) -> Self {
        let queue = Arc::new(queue);
        let done = Arc::new(AtomicBool::new(false));

        let q = queue.clone();
        let d = done.clone();

        wasm_bindgen_futures::spawn_local(async move {
            let reader = stream
                .get_reader()
                .unchecked_into::<web_sys::ReadableStreamDefaultReader>();

            let mut pending: Option<JsStreamValue> = None;

            loop {
                // Drain any cached item from a previous full-queue push before
                // reading the next chunk from JS.
                if let Some(item) = pending.take() {
                    match q.push(item) {
                        Ok(()) => {}
                        Err(concurrent_queue::PushError::Closed(_)) => {
                            d.store(true, Ordering::Release);
                            break;
                        }
                        Err(concurrent_queue::PushError::Full(rejected)) => {
                            pending = Some(rejected);
                            continue;
                        }
                    }
                }

                let result = match JsFuture::from(reader.read()).await {
                    Ok(val) => val,
                    Err(js_err) => {
                        let _ = q.push(JsStreamValue::Error(format!("{js_err:?}")));
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
                        match q.push(JsStreamValue::Chunk(bytes)) {
                            Ok(()) => {}
                            Err(concurrent_queue::PushError::Closed(_)) => {
                                d.store(true, Ordering::Release);
                                break;
                            }
                            Err(concurrent_queue::PushError::Full(rejected)) => {
                                pending = Some(rejected);
                            }
                        }
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
            Ok(item) => Some(item),
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
/// One item that a byte source gives to a JS `ReadableStream` pull.
pub enum JsStreamItem {
    /// Bytes to enqueue now.
    Chunk(Vec<u8>),
    /// The source has nothing yet. The pull parks until readiness or a timer.
    Pending,
    /// The source failed. The pull fails the stream with this message.
    Error(String),
}

/// A waker that resolves a JS promise once.
struct PromiseWake {
    resolve: std::sync::Mutex<Option<foundation_compact::SendWrapper<js_sys::Function>>>,
}

impl std::task::Wake for PromiseWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let resolve = self.resolve.lock().ok().and_then(|mut slot| slot.take());
        if let Some(resolve) = resolve {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        }
    }
}

/// Wrap a byte source in a JS `ReadableStream` for use as a `fetch()` request
/// body.
///
/// A `Pending` item parks the pull and returns a promise. When `readiness` can
/// wake, the wake resolves the promise; otherwise a `setTimeout(0)` does. The
/// pull never calls the source again in a loop (root rule 6).
///
/// # Errors
///
/// The `JsValue` error the runtime raised while building the stream.
pub fn iterator_to_readable_stream(
    source: Box<dyn Iterator<Item = JsStreamItem>>,
    readiness: Option<Arc<dyn crate::valtron::EventReadiness + Send + Sync>>,
) -> Result<web_sys::ReadableStream, JsValue> {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    let source = Rc::new(RefCell::new(Some(source)));
    let cancelled = Rc::new(Cell::new(false));

    let pull_source = Rc::clone(&source);
    let pull_cancelled = Rc::clone(&cancelled);
    let pull_readiness = readiness.clone();
    let pull = Closure::wrap(Box::new(move |controller: JsValue| -> js_sys::Promise {
        let controller: web_sys::ReadableStreamDefaultController = controller.unchecked_into();
        let source = Rc::clone(&pull_source);
        let cancelled = Rc::clone(&pull_cancelled);
        let readiness = pull_readiness.clone();
        // The pull owns the wait loop. A `ReadableStream` does not pull again
        // when a pull promise resolves without enqueuing, so this future
        // awaits each `Pending` and asks the source again. Each wait is an
        // awaited promise, never a synchronous loop (root rule 6).
        wasm_bindgen_futures::future_to_promise(async move {
            loop {
                if cancelled.get() {
                    let _ = controller.close();
                    return Ok(JsValue::UNDEFINED);
                }
                // The borrow ends before the await; only the item survives.
                let item = {
                    let mut slot = source.borrow_mut();
                    slot.as_mut().and_then(|source| source.next())
                };
                match item {
                    Some(JsStreamItem::Chunk(bytes)) => {
                        let array = js_sys::Uint8Array::from(bytes.as_slice());
                        let _ = controller.enqueue_with_chunk(&array.into());
                        return Ok(JsValue::UNDEFINED);
                    }
                    Some(JsStreamItem::Pending) => {
                        let parked = park_pull(readiness.as_ref());
                        let _ = JsFuture::from(parked).await;
                    }
                    Some(JsStreamItem::Error(message)) => {
                        let _ = controller.error_with_e(&JsValue::from_str(&message));
                        return Ok(JsValue::UNDEFINED);
                    }
                    None => {
                        let _ = controller.close();
                        return Ok(JsValue::UNDEFINED);
                    }
                }
            }
        })
    }) as Box<dyn FnMut(JsValue) -> js_sys::Promise>);

    let cancel = {
        let source = Rc::clone(&source);
        let cancelled = Rc::clone(&cancelled);
        Closure::wrap(Box::new(move |_reason: JsValue| {
            cancelled.set(true);
            source.borrow_mut().take();
        }) as Box<dyn FnMut(JsValue)>)
    };

    // Build the underlying source as a plain object: this crate does not
    // enable web-sys' `UnderlyingSource` feature, and `pull`/`cancel` are
    // ordinary properties.
    let underlying = js_sys::Object::new();
    js_sys::Reflect::set(
        &underlying,
        &JsValue::from_str("pull"),
        pull.as_ref().unchecked_ref(),
    )?;
    js_sys::Reflect::set(
        &underlying,
        &JsValue::from_str("cancel"),
        cancel.as_ref().unchecked_ref(),
    )?;

    // The stream holds the closures for its lifetime; the JS heap owns them.
    pull.forget();
    cancel.forget();

    web_sys::ReadableStream::new_with_underlying_source(&underlying)
}

/// A promise that resolves when the source may have data.
///
/// It registers a waker with the readiness. When the readiness cannot wake, or
/// there is no readiness, a `setTimeout(0)` resolves it so the event loop turns.
fn park_pull(
    readiness: Option<&Arc<dyn crate::valtron::EventReadiness + Send + Sync>>,
) -> js_sys::Promise {
    js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(readiness) = readiness {
            let wake = Arc::new(PromiseWake {
                resolve: std::sync::Mutex::new(Some(foundation_compact::SendWrapper::new(
                    resolve.clone(),
                ))),
            });
            let waker = std::task::Waker::from(wake);
            let readiness_waker = crate::valtron::ReadinessWaker::from_waker(waker);
            if readiness.register(&readiness_waker) {
                // The source may have become ready between the `Pending` item
                // and the registration. Re-check once, so no wake is lost.
                if readiness.is_ready(None) {
                    readiness_waker.wake();
                }
                return;
            }
        }
        // No readiness, or it cannot wake: resolve on the next macrotask.
        resolve_soon(&resolve);
    })
}

/// Resolve a JS function on the next macrotask.
fn resolve_soon(resolve: &js_sys::Function) {
    let global = js_sys::global();
    let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
        .ok()
        .and_then(|value| value.dyn_ref::<js_sys::Function>().cloned());
    if let Some(set_timeout) = set_timeout {
        let _ = set_timeout.call2(&global, resolve, &JsValue::from_f64(0.0));
    } else {
        // No timer: resolve now, so the caller is not stuck.
        let _ = resolve.call0(&JsValue::UNDEFINED);
    }
}
