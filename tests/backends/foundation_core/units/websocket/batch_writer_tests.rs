#![cfg(test)]

//! Batch frame writer tests.

use foundation_core::wire::websocket::{BatchFrameWriter, Opcode, WebSocketFrame};
use std::time::Duration;

fn text_frame(payload: &[u8], fin: bool) -> WebSocketFrame {
    WebSocketFrame {
        fin,
        opcode: Opcode::Text,
        mask: None,
        payload: payload.to_vec(),
    }
}

#[test]
fn test_batch_writer_with_defaults() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::with_defaults(&mut buffer);

    let frame = text_frame(b"hello", true);
    writer.queue_frame(frame).unwrap();

    // Should not be flushed yet (below threshold)
    assert_eq!(writer.queued_frame_count(), 1);
    assert!(writer.buffered_bytes() > 0);

    writer.flush().unwrap();

    // Should be flushed now
    assert_eq!(writer.queued_frame_count(), 0);
    assert_eq!(writer.buffered_bytes(), 0);
    assert!(!buffer.is_empty());
}

#[test]
fn test_batch_writer_custom_limits() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::new(
        &mut buffer,
        1024, // 1 KiB
        Duration::from_millis(100),
    );

    let frame = text_frame(b"test message", true);
    writer.queue_frame(frame).unwrap();

    assert_eq!(writer.queued_frame_count(), 1);
    writer.flush().unwrap();
    assert_eq!(writer.queued_frame_count(), 0);
}

#[test]
fn test_batch_writer_auto_flush_on_size() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::new(
        &mut buffer,
        50,                      // Small limit for testing
        Duration::from_secs(10), // Long timeout
    );

    // Queue frames until auto-flush triggers
    for _ in 0..10 {
        let frame = text_frame(b"hello world!", true); // ~17 bytes encoded
        writer.queue_frame(frame).unwrap();
    }

    // Should have auto-flushed at least once
    assert!(writer.stats().flushes > 0);
}

#[test]
fn test_write_immediate() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::with_defaults(&mut buffer);

    // Queue a frame
    let frame1 = text_frame(b"batched", true);
    writer.queue_frame(frame1).unwrap();
    assert_eq!(writer.queued_frame_count(), 1);

    // Write immediate should flush existing batch first
    let frame2 = text_frame(b"immediate", true);
    writer.write_immediate(frame2).unwrap();

    // Both frames should be written
    assert_eq!(writer.stats().frames, 2);
    assert_eq!(writer.queued_frame_count(), 0);
    assert!(!buffer.is_empty());
}

#[test]
fn test_queue_multiple_frames() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::with_defaults(&mut buffer);

    let frames = vec![
        text_frame(b"message 1", true),
        text_frame(b"message 2", true),
        text_frame(b"message 3", true),
    ];

    writer.queue_frames(frames).unwrap();

    assert_eq!(writer.queued_frame_count(), 3);
    writer.flush().unwrap();
    assert_eq!(writer.queued_frame_count(), 0);
    assert_eq!(writer.stats().frames, 3);
}

#[test]
fn test_stats_tracking() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::with_defaults(&mut buffer);

    let frame = text_frame(b"test", true);
    let frame_size = frame.encode().len();

    writer.queue_frame(frame.clone()).unwrap();
    writer.queue_frame(frame.clone()).unwrap();
    writer.queue_frame(frame).unwrap();
    writer.flush().unwrap();

    let stats = writer.stats();
    assert_eq!(stats.frames, 3);
    assert_eq!(stats.flushes, 1);
    assert_eq!(stats.bytes, frame_size * 3);
    assert_eq!(stats.buffered_bytes, 0);
    assert_eq!(stats.queued_frames, 0);
}

#[test]
fn test_flush_efficiency() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::with_defaults(&mut buffer);

    let frame = text_frame(b"test data", true);

    // Queue multiple frames before flushing
    for _ in 0..5 {
        writer.queue_frame(frame.clone()).unwrap();
    }
    writer.flush().unwrap();

    let stats = writer.stats();
    // Flush efficiency should be > 1 (multiple frames per flush)
    assert!(stats.flush_efficiency() > 1.0);
}

#[test]
fn test_empty_flush_is_noop() {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = BatchFrameWriter::with_defaults(&mut buffer);

    // Flushing empty buffer should be a no-op
    writer.flush().unwrap();
    assert_eq!(writer.stats().flushes, 0);
    assert!(buffer.is_empty());
}

#[test]
fn test_into_inner_flushes() {
    let mut buffer: Vec<u8> = Vec::new();
    let buffer_len;

    {
        let frame = text_frame(b"final", true);

        // Create writer, queue frame, and get inner
        let writer = BatchFrameWriter::with_defaults(&mut buffer);
        let mut writer = writer; // Make mutable for into_inner
        writer.queue_frame(frame).unwrap();
        let _inner = writer.into_inner().unwrap();
        // After into_inner, buffer should be flushed
        buffer_len = buffer.len();
    }

    // Buffer should have data after flush
    assert!(buffer_len > 0);
}
