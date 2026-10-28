//! A `Read` adapter over a `ConcurrentQueue<Vec<u8>>` with a shared done flag.
//!
//! WHY: `SseParser` requires `std::io::Read`. On wasm, SSE bytes arrive
//! asynchronously from a JS `ReadableStream` reader task, which pushes chunks
//! into a `ConcurrentQueue`. This adapter turns that queue into a `Read`.
//!
//! WHAT: Buffers one chunk at a time. When the current chunk is exhausted it
//! pops the next one from the queue. If the queue is temporarily empty but the
//! producer is not done, it returns `ErrorKind::WouldBlock` so the parser parks
//! and resumes when more data arrives. It returns `Ok(0)` (end of stream) only
//! when the queue is empty and the producer has ended.
//!
//! WHY `WouldBlock` matters: `Ok(0)` is the end-of-stream signal. A `data:` line
//! split across two chunks would otherwise parse as a complete line and emit a
//! partial JSON payload (the "Failed to parse SSE chunk" bug).
//!
//! HOW: Holds `Arc<ConcurrentQueue>` shared with the async producer and an
//! `Arc<AtomicBool>` `done` flag that the producer sets on close. The cursor
//! tracks position within the current chunk.

use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;

/// A `Read` view over a `ConcurrentQueue<Vec<u8>>` with a shared done flag.
pub struct QueueReader {
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    done: Arc<AtomicBool>,
    current: Cursor<Vec<u8>>,
}

impl QueueReader {
    /// A reader over `queue` that ends only after `done` is set.
    pub fn new(queue: Arc<ConcurrentQueue<Vec<u8>>>, done: Arc<AtomicBool>) -> Self {
        Self {
            queue,
            done,
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
            // `queue.pop()` returns `Err(concurrent_queue::Empty)` when the queue
            // has no chunk yet. `Empty` is a sentinel, not a failure, and carries
            // no payload to log; it fires on every poll while the producer is in
            // flight, so logging it as an error would flood the log and mislabel
            // the normal "waiting for data" path. When the producer is not done,
            // surface it as `WouldBlock` so `SseParser` parks instead of treating
            // a `data:` line split across two chunks as a complete line. Only when
            // the producer has ended is an empty queue true EOF (`Ok(0)`).
            Err(_) if !self.done.load(Ordering::Acquire) => {
                Err(std::io::ErrorKind::WouldBlock.into())
            }
            Err(_) => Ok(0),
        }
    }
}

/// SSE events parsed from a [`QueueReader`]'s queue, as valtron stream items.
///
/// WHY: the wasm SSE path (`WasmSseIterator`) feeds a queue from a JS
/// `ReadableStream`. Its "what to yield" logic lives here, target-independent,
/// so it is tested natively.
///
/// WHAT: `Next(event)` per parsed event; `Wait` while the queue is empty and
/// the producer is still running (park on [`WakeableStream::wake`]); a failed
/// read or parse ends the stream with `Pending(SseProgress::Failed(msg))` then
/// `None` — a malformed feed never recovers, so waiting on it would stall the
/// consumer forever; end of stream once the producer is done and the queue
/// is drained.
///
/// HOW: the producer pushes chunks onto `queue`, sets `done` when it ends, and
/// fires `wake` after each push and at the end.
pub struct QueueSseEvents<R: Read = QueueReader> {
    parser: crate::event_source::SseParser<R>,
    done: Arc<AtomicBool>,
    queue: Arc<ConcurrentQueue<Vec<u8>>>,
    wake: Arc<foundation_core::valtron::WakeRegistry>,
    failed: bool,
}

impl QueueSseEvents {
    /// Events from `queue`, which the producer ends by setting `done`; the
    /// producer fires `wake` whenever it pushes a chunk or ends.
    #[must_use]
    pub fn new(
        queue: Arc<ConcurrentQueue<Vec<u8>>>,
        done: Arc<AtomicBool>,
        wake: Arc<foundation_core::valtron::WakeRegistry>,
    ) -> Self {
        let reader = QueueReader::new(queue.clone(), done.clone());
        Self::over_reader(reader, queue, done, wake)
    }
}

impl<R: Read> QueueSseEvents<R> {
    /// Events parsed from `reader`, a reader over `queue` (normally a
    /// [`QueueReader`]); `queue` and `done` tell "not yet" from "ended".
    #[must_use]
    pub fn over_reader(
        reader: R,
        queue: Arc<ConcurrentQueue<Vec<u8>>>,
        done: Arc<AtomicBool>,
        wake: Arc<foundation_core::valtron::WakeRegistry>,
    ) -> Self {
        let buffer = foundation_core::io::ioutils::SharedByteBufferStream::ref_cell(reader);
        Self {
            parser: crate::event_source::SseParser::new(buffer),
            done,
            queue,
            wake,
            failed: false,
        }
    }

    fn drained(&self) -> bool {
        self.done.load(Ordering::Acquire) && self.queue.is_empty()
    }
}

impl<R: Read> Iterator for QueueSseEvents<R> {
    type Item = foundation_core::valtron::Stream<
        crate::event_source::ParseResult,
        crate::shared::client::http_client::SseProgress,
    >;

    fn next(&mut self) -> Option<Self::Item> {
        use crate::event_source::EventSourceError;
        use foundation_core::valtron::Stream;

        if self.failed {
            return None;
        }
        match self.parser.parse_next() {
            Ok(Some(result)) => Some(Stream::Next(result)),
            Ok(None) if self.drained() => None,
            Ok(None) => Some(Stream::Wait),
            // `WouldBlock` is the "no data yet" parking signal, not an error.
            Err(EventSourceError::ReadError(err))
                if err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                if self.drained() {
                    None
                } else {
                    Some(Stream::Wait)
                }
            }
            Err(e) => {
                tracing::error!("SSE stream failed: {e}");
                self.failed = true;
                Some(Stream::Pending(
                    crate::shared::client::http_client::SseProgress::Failed(e.to_string()),
                ))
            }
        }
    }
}

impl<R: Read> foundation_core::valtron::WakeableStream for QueueSseEvents<R> {
    fn wake(
        &mut self,
        cb: foundation_core::valtron::WakeCallback,
    ) -> foundation_core::valtron::WakeResult {
        self.wake.register(cb);
        // Register-then-check: if a chunk (or the end) is already waiting, no
        // later push will fire the registry, so fire it now.
        if self.failed || !self.queue.is_empty() || self.done.load(Ordering::Acquire) {
            self.wake.fire();
        }
        foundation_core::valtron::WakeResult::Successful
    }
}
