//! WHY: F28 — `StreamRegistry` must correctly create, send to, and close
//! streams using `ConcurrentQueue` for wasm→host streaming.
//!
//! WHAT: create → send chunks → close; verify queue receives chunks in order.
//!
//! HOW: native tests using `ConcurrentQueue` (no wasm needed for the
//! registry logic — it's platform-agnostic).

use foundation_wasm::stream::{StreamChunk, StreamId, StreamRegistry};

#[test]
fn create_and_close_stream() {
    let reg = StreamRegistry::new();
    let (id, _queue) = reg.create();
    assert!(id.0 > 0);
    assert!(reg.close(id));
    assert!(!reg.close(id)); // already closed
}

#[test]
fn send_to_stream_and_drain() {
    let reg = StreamRegistry::new();
    let (id, queue) = reg.create();

    assert!(reg.send(id, StreamChunk::new(b"hello".to_vec(), 0)));
    assert!(reg.send(id, StreamChunk::new(b"world".to_vec(), 1)));

    // Drain the queue
    let c1 = queue.pop().unwrap();
    assert_eq!(c1.data, b"hello");
    assert_eq!(c1.sequence, 0);

    let c2 = queue.pop().unwrap();
    assert_eq!(c2.data, b"world");
    assert_eq!(c2.sequence, 1);

    // Close stops further sends
    reg.close(id);
    assert!(!reg.send(id, StreamChunk::new(b"late".to_vec(), 2)));
}

#[test]
fn send_to_nonexistent_stream() {
    let reg = StreamRegistry::new();
    assert!(!reg.send(StreamId(999), StreamChunk::new(b"x".to_vec(), 0)));
    assert!(!reg.close(StreamId(999)));
}

#[test]
fn multiple_streams_independent() {
    let reg = StreamRegistry::new();
    let (id1, q1) = reg.create();
    let (id2, q2) = reg.create();

    assert!(reg.send(id1, StreamChunk::new(b"a".to_vec(), 0)));
    assert!(reg.send(id2, StreamChunk::new(b"b".to_vec(), 0)));

    assert_eq!(q1.pop().unwrap().data, b"a");
    assert_eq!(q2.pop().unwrap().data, b"b");

    reg.close(id1);
    assert!(reg.send(id2, StreamChunk::new(b"c".to_vec(), 1)));
    assert!(!reg.send(id1, StreamChunk::new(b"d".to_vec(), 1)));
}

#[test]
fn stream_ids_monotonically_increasing() {
    let reg = StreamRegistry::new();
    let (id1, _) = reg.create();
    let (id2, _) = reg.create();
    let (id3, _) = reg.create();
    assert!(id1.0 < id2.0);
    assert!(id2.0 < id3.0);
}
