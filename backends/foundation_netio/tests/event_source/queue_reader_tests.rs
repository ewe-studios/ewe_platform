//! Tests for the queue-to-`Read` adapter used by the wasm SSE path.
//!
//! WHY: On wasm, `WasmSseIterator` feeds `SseParser` from a JS `ReadableStream`
//! through `QueueReader`. A `data:` line split across two JS chunks must not
//! parse as a complete line. `QueueReader` must return `WouldBlock` while the
//! queue is empty and the producer is not done, and `Ok(0)` (end of stream)
//! only once the producer is done.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::event_source::{Event, EventSourceError, SseParser};
use foundation_netio::shared::client::queue_reader::QueueReader;

fn reader_and_queue() -> (QueueReader, Arc<ConcurrentQueue<Vec<u8>>>, Arc<AtomicBool>) {
    let queue = Arc::new(ConcurrentQueue::unbounded());
    let done = Arc::new(AtomicBool::new(false));
    let reader = QueueReader::new(queue.clone(), done.clone());
    (reader, queue, done)
}

/// An empty queue while the producer is open is "wait", not end of stream.
#[test]
fn empty_queue_while_open_is_would_block_not_eof() {
    let (mut reader, _queue, _done) = reader_and_queue();
    let mut buf = [0u8; 32];
    let err = reader.read(&mut buf).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
}

/// An empty queue after the producer has ended is end of stream.
#[test]
fn empty_queue_after_done_is_eof() {
    let (mut reader, _queue, done) = reader_and_queue();
    done.store(true, Ordering::Release);
    let mut buf = [0u8; 32];
    assert_eq!(reader.read(&mut buf).unwrap(), 0);
}

/// A pushed chunk is returned whole.
#[test]
fn reads_a_pushed_chunk() {
    let (mut reader, queue, _done) = reader_and_queue();
    queue.push(b"hello".to_vec()).unwrap();
    let mut buf = [0u8; 32];
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello");
}

/// A chunk larger than the buffer is returned across several reads.
#[test]
fn reads_a_large_chunk_in_parts() {
    let (mut reader, queue, _done) = reader_and_queue();
    queue.push(b"0123456789".to_vec()).unwrap();
    let mut buf = [0u8; 4];

    assert_eq!(reader.read(&mut buf).unwrap(), 4);
    assert_eq!(&buf, b"0123");

    assert_eq!(reader.read(&mut buf).unwrap(), 4);
    assert_eq!(&buf, b"4567");

    assert_eq!(reader.read(&mut buf).unwrap(), 2);
    assert_eq!(&buf[..2], b"89");
}

/// A `data:` line split across two pushes must not parse as a complete line.
///
/// This is the exact "Failed to parse SSE chunk" bug: before the fix,
/// `QueueReader` returned `Ok(0)` on an empty queue, so the parser dispatched
/// the half-line as a complete event.
#[test]
fn sse_parser_waits_for_a_complete_line_across_chunks() {
    let queue = Arc::new(ConcurrentQueue::unbounded());
    let done = Arc::new(AtomicBool::new(false));
    let reader = QueueReader::new(queue.clone(), done.clone());
    let buffer = SharedByteBufferStream::ref_cell(reader);
    let mut parser = SseParser::new(buffer);

    // First chunk: a `data:` line with no newline yet. The parser must park,
    // not emit a partial event.
    queue.push(b"data: hello".to_vec()).unwrap();
    match parser.parse_next() {
        Err(EventSourceError::ReadError(err)) => {
            assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
        }
        other => panic!("expected WouldBlock, got {other:?}"),
    }

    // Second chunk completes the line and the blank separator.
    queue.push(b"\n\n".to_vec()).unwrap();
    match parser.parse_next().unwrap() {
        Some(result) => match result.event {
            Event::Message { data, .. } => assert_eq!(data, "hello"),
            other => panic!("expected Message, got {other:?}"),
        },
        None => panic!("expected a complete event"),
    }
}
