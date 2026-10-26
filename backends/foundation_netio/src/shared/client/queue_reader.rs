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
