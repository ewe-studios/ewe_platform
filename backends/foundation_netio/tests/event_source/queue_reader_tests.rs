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

/// The fields of an event read before the queue ran dry must survive the
/// `WouldBlock`: the event is dispatched by the blank line that arrives later.
/// The parser used to keep its event builder in a local, so the `event:` and
/// `data:` lines read before the park were dropped and the blank line
/// dispatched nothing.
#[test]
fn sse_parser_keeps_an_event_s_fields_across_a_park() {
    let queue = Arc::new(ConcurrentQueue::unbounded());
    let done = Arc::new(AtomicBool::new(false));
    let reader = QueueReader::new(queue.clone(), done.clone());
    let buffer = SharedByteBufferStream::ref_cell(reader);
    let mut parser = SseParser::new(buffer);

    queue.push(b"event: greeting\ndata: hello\n".to_vec()).unwrap();
    match parser.parse_next() {
        Err(EventSourceError::ReadError(err)) => {
            assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
        }
        other => panic!("expected WouldBlock, got {other:?}"),
    }

    queue.push(b"\n".to_vec()).unwrap();
    match parser.parse_next().unwrap() {
        Some(result) => match result.event {
            Event::Message { data, event_type, .. } => {
                assert_eq!(data, "hello");
                assert_eq!(event_type.as_deref(), Some("greeting"));
            }
            other => panic!("expected Message, got {other:?}"),
        },
        None => panic!("expected the event read before the park"),
    }
}

/// A feed that fails to read ends the stream with `Pending(SseProgress::Failed)`
/// and then `None`. It used to answer `Wait` for ever, so a broken feed stalled
/// its consumer instead of failing.
#[test]
fn queue_sse_events_fail_a_broken_feed_instead_of_waiting() {
    use foundation_core::valtron::{Stream, WakeRegistry};
    use foundation_netio::shared::client::http_client::SseProgress;
    use foundation_netio::shared::client::queue_reader::QueueSseEvents;

    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "malformed feed"))
        }
    }

    let queue = Arc::new(ConcurrentQueue::unbounded());
    queue.push(b"data: x".to_vec()).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let mut events =
        QueueSseEvents::over_reader(Broken, queue, done, Arc::new(WakeRegistry::new()));

    match events.next() {
        Some(Stream::Pending(SseProgress::Failed(msg))) => assert!(!msg.is_empty()),
        other => panic!("expected Pending(Failed), got {other:?}"),
    }
    assert!(events.next().is_none(), "a failed stream ends");
}

/// An open, empty queue parks (`Wait`); a drained, finished one ends.
#[test]
fn queue_sse_events_wait_while_open_and_end_when_done() {
    use foundation_core::valtron::{Stream, WakeRegistry};
    use foundation_netio::shared::client::queue_reader::QueueSseEvents;

    let queue = Arc::new(ConcurrentQueue::unbounded());
    let done = Arc::new(AtomicBool::new(false));
    let mut events =
        QueueSseEvents::new(queue.clone(), done.clone(), Arc::new(WakeRegistry::new()));

    assert!(matches!(events.next(), Some(Stream::Wait)));
    queue.push(b"data: hi\n\n".to_vec()).unwrap();
    assert!(matches!(events.next(), Some(Stream::Next(_))));
    done.store(true, Ordering::Release);
    assert!(events.next().is_none());
}

/// A producer-recorded read error ends the stream with
/// `Pending(SseProgress::Failed)` and then `None`, not a clean end. This is the
/// wasm path: the JS `ReadableStream` read errors after one event, and the error
/// travels beside the `done` flag.
#[test]
fn queue_sse_events_surface_a_producer_read_error() {
    use std::sync::Mutex;

    use foundation_core::valtron::{Stream, WakeRegistry};
    use foundation_netio::shared::client::http_client::SseProgress;
    use foundation_netio::shared::client::queue_reader::QueueSseEvents;

    let queue = Arc::new(ConcurrentQueue::unbounded());
    let done = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None));
    let mut events = QueueSseEvents::new_with_error(
        queue.clone(),
        done.clone(),
        error.clone(),
        Arc::new(WakeRegistry::new()),
    );

    // The producer pushes one event, then records a read error and ends.
    queue.push(b"data: hello\n\n".to_vec()).unwrap();
    *error.lock().unwrap() = Some("read error".to_string());
    done.store(true, Ordering::Release);

    // The queued event comes first.
    assert!(matches!(events.next(), Some(Stream::Next(_))));

    // Then the recorded error, not a clean end.
    match events.next() {
        Some(Stream::Pending(SseProgress::Failed(msg))) => assert_eq!(msg, "read error"),
        other => panic!("expected Pending(Failed), got {other:?}"),
    }
    assert!(events.next().is_none(), "a failed stream ends");
}
