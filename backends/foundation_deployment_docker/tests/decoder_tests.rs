//! Unit tests for Docker streaming decoders.
//!
//! WHY: Prove `LogFrameDecoder` and `JsonLineDecoder` correctly handle complete
//! frames, partial frames (buffered across `feed()` calls), multiple frames in
//! one chunk, and edge cases — without a running Docker daemon.
//!
//! WHAT: Pure unit tests — no network, no valtron pool needed.

#![cfg(feature = "docker")]

use foundation_deployment_docker::streaming::decoder::{JsonLineDecoder, LogFrameDecoder, LogOutput};

// =============================================================================
// LogFrameDecoder
// =============================================================================

#[test]
fn log_decoder_single_stdout_frame() {
    let mut dec = LogFrameDecoder::new();
    // stream_type=1(stdout), padding=[0,0,0], length=5, payload="hello"
    let mut frame = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05];
    frame.extend_from_slice(b"hello");
    dec.feed(&frame);

    let out = dec.decode().expect("should decode complete frame");
    assert_eq!(out, LogOutput::StdOut { message: bytes::Bytes::from("hello") });
    assert!(dec.decode().is_none(), "no more frames");
    assert_eq!(dec.buffered_len(), 0);
}

#[test]
fn log_decoder_single_stderr_frame() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04];
    frame.extend_from_slice(b"oops");
    dec.feed(&frame);

    let out = dec.decode().expect("should decode");
    assert_eq!(out, LogOutput::StdErr { message: bytes::Bytes::from("oops") });
}

#[test]
fn log_decoder_stdin_frame() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    frame.extend_from_slice(b"foo");
    dec.feed(&frame);

    let out = dec.decode().expect("should decode");
    assert_eq!(out, LogOutput::StdIn { message: bytes::Bytes::from("foo") });
}

#[test]
fn log_decoder_console_frame() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];
    frame.extend_from_slice(b"hi");
    dec.feed(&frame);

    let out = dec.decode().expect("should decode");
    assert_eq!(out, LogOutput::Console { message: bytes::Bytes::from("hi") });
}

#[test]
fn log_decoder_partial_header_returns_none() {
    let mut dec = LogFrameDecoder::new();
    // Only 4 bytes — not enough for the 8-byte header
    dec.feed(&[0x01, 0x00, 0x00, 0x00]);
    assert!(dec.decode().is_none(), "partial header should return None");
    assert_eq!(dec.buffered_len(), 4);
}

#[test]
fn log_decoder_partial_payload_returns_none() {
    let mut dec = LogFrameDecoder::new();
    // Header says 20 bytes but only 5 bytes of payload provided
    let mut chunk = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14];
    chunk.extend_from_slice(b"short");
    dec.feed(&chunk);
    assert!(dec.decode().is_none(), "partial payload should return None");
    assert_ne!(dec.buffered_len(), 0);
}

#[test]
fn log_decoder_multiple_frames_single_chunk() {
    let mut dec = LogFrameDecoder::new();
    // Frame 1: stdout "ab"
    let mut chunk = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];
    chunk.extend_from_slice(b"ab");
    // Frame 2: stderr "cd"
    chunk.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]);
    chunk.extend_from_slice(b"cd");
    dec.feed(&chunk);

    let f1 = dec.decode().expect("frame 1");
    assert_eq!(f1, LogOutput::StdOut { message: bytes::Bytes::from("ab") });
    let f2 = dec.decode().expect("frame 2");
    assert_eq!(f2, LogOutput::StdErr { message: bytes::Bytes::from("cd") });
    assert!(dec.decode().is_none(), "no more frames");
}

#[test]
fn log_decoder_incremental_feeds() {
    let mut dec = LogFrameDecoder::new();
    // Feed header only
    dec.feed(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);
    assert!(dec.decode().is_none());

    // Feed payload
    dec.feed(b"xyz");
    let out = dec.decode().expect("frame after incremental feed");
    assert_eq!(out, LogOutput::StdOut { message: bytes::Bytes::from("xyz") });
}

#[test]
fn log_decoder_compact_frees_memory() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    frame.extend_from_slice(b"foo");
    dec.feed(&frame);
    let _ = dec.decode();
    assert_eq!(dec.buffered_len(), 0, "all bytes decoded");

    // Feed more data and decode
    let mut frame2 = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    frame2.extend_from_slice(b"bar");
    dec.feed(&frame2);
    let _ = dec.decode();
    assert_eq!(dec.buffered_len(), 0);
}

#[test]
fn log_decoder_large_payload() {
    let mut dec = LogFrameDecoder::new();
    let payload = vec![b'x'; 65536];
    let mut frame = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]; // length = 65536
    frame.extend_from_slice(&payload);
    dec.feed(&frame);

    let out = dec.decode().expect("large frame");
    assert_eq!(out, LogOutput::StdOut { message: bytes::Bytes::from(payload) });
}

// =============================================================================
// JsonLineDecoder
// =============================================================================

#[test]
fn json_line_decoder_single_line() {
    let mut dec = JsonLineDecoder::new();
    dec.feed(b"{\"status\":\"ok\"}\n");
    let line = dec.decode().expect("should decode complete line");
    assert_eq!(line, bytes::Bytes::from("{\"status\":\"ok\"}"));
    assert!(dec.decode().is_none());
}

#[test]
fn json_line_decoder_multiple_lines_single_chunk() {
    let mut dec = JsonLineDecoder::new();
    dec.feed(b"{\"a\":1}\n{\"b\":2}\n");

    let l1 = dec.decode().expect("line 1");
    assert_eq!(l1, bytes::Bytes::from("{\"a\":1}"));
    let l2 = dec.decode().expect("line 2");
    assert_eq!(l2, bytes::Bytes::from("{\"b\":2}"));
    assert!(dec.decode().is_none());
}

#[test]
fn json_line_decoder_partial_line_returns_none() {
    let mut dec = JsonLineDecoder::new();
    dec.feed(b"{\"partial\":");
    assert!(dec.decode().is_none(), "no newline yet");
    assert_eq!(dec.buffered_len(), 11);
}

#[test]
fn json_line_decoder_incremental_feeds() {
    let mut dec = JsonLineDecoder::new();
    dec.feed(b"{\"status\":"); // partial
    assert!(dec.decode().is_none());
    dec.feed(b"\"running\"}\n"); // complete
    let line = dec.decode().expect("complete after feed");
    assert_eq!(line, bytes::Bytes::from("{\"status\":\"running\"}"));
}

#[test]
fn json_line_decoder_empty_lines_skipped() {
    let mut dec = JsonLineDecoder::new();
    dec.feed(b"\n\n{\"key\":\"val\"}\n\n");
    let line = dec.decode().expect("line after empty");
    assert_eq!(line, bytes::Bytes::from("{\"key\":\"val\"}"));
    assert!(dec.decode().is_none());
}

#[test]
fn json_line_decoder_compact_frees_memory() {
    let mut dec = JsonLineDecoder::new();
    dec.feed(b"line1\nline2\n");
    // Decode both lines first — compact only works on already-decoded data
    let _l1 = dec.decode().expect("line1");
    let _l2 = dec.decode().expect("line2");
    dec.compact();
    assert_eq!(dec.buffered_len(), 0);

    dec.feed(b"line3\n");
    let l3 = dec.decode().expect("line3 after compact");
    assert_eq!(l3, bytes::Bytes::from("line3"));
}
