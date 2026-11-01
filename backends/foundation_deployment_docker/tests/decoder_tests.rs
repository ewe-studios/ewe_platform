//! Unit tests for Docker streaming decoders.
//!
//! WHY: Prove `LogFrameDecoder` correctly handles complete
//! frames, partial frames (buffered across `feed()` calls), multiple frames in
//! one chunk, and edge cases — without a running Docker daemon.
//!
//! WHAT: Pure unit tests — no network, no valtron pool needed.

#![cfg(feature = "docker")]

use foundation_deployment_docker::streaming::decoder::{DecodeOutcome, LogFrameDecoder, LogOutput};

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

    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from("hello") })
    );
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore, "no more frames");
    assert_eq!(dec.buffered_len(), 0);
}

#[test]
fn log_decoder_single_stderr_frame() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04];
    frame.extend_from_slice(b"oops");
    dec.feed(&frame);

    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdErr { message: bytes::Bytes::from("oops") })
    );
}

#[test]
fn log_decoder_stdin_frame() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    frame.extend_from_slice(b"foo");
    dec.feed(&frame);

    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdIn { message: bytes::Bytes::from("foo") })
    );
}

#[test]
fn log_decoder_console_frame() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];
    frame.extend_from_slice(b"hi");
    dec.feed(&frame);

    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::Console { message: bytes::Bytes::from("hi") })
    );
}

#[test]
fn log_decoder_partial_header_returns_need_more() {
    let mut dec = LogFrameDecoder::new();
    // Only 4 bytes — not enough for the 8-byte header
    dec.feed(&[0x01, 0x00, 0x00, 0x00]);
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore);
    assert_eq!(dec.buffered_len(), 4);
}

#[test]
fn log_decoder_partial_payload_returns_need_more() {
    let mut dec = LogFrameDecoder::new();
    // Header says 20 bytes but only 5 bytes of payload provided
    let mut chunk = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14];
    chunk.extend_from_slice(b"short");
    dec.feed(&chunk);
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore);
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

    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from("ab") })
    );
    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdErr { message: bytes::Bytes::from("cd") })
    );
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore, "no more frames");
}

#[test]
fn log_decoder_incremental_feeds() {
    let mut dec = LogFrameDecoder::new();
    // Feed header only
    dec.feed(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore);

    // Feed payload
    dec.feed(b"xyz");
    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from("xyz") })
    );
}

#[test]
fn log_decoder_compact_frees_memory() {
    let mut dec = LogFrameDecoder::new();
    let mut frame = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    frame.extend_from_slice(b"foo");
    dec.feed(&frame);
    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from("foo") })
    );
    assert_eq!(dec.buffered_len(), 0, "all bytes decoded");

    // Feed more data and decode
    let mut frame2 = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    frame2.extend_from_slice(b"bar");
    dec.feed(&frame2);
    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from("bar") })
    );
    assert_eq!(dec.buffered_len(), 0);
}

#[test]
fn log_decoder_large_payload() {
    let mut dec = LogFrameDecoder::new();
    let payload = vec![b'x'; 65536];
    let mut frame = vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]; // length = 65536
    frame.extend_from_slice(&payload);
    dec.feed(&frame);

    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from(payload) })
    );
}

#[test]
fn log_frame_decoder_distinguishes_need_more_from_skip() {
    // WHY: `NeedMore` and `Skipped` used to both read as `None`, so a caller
    // could not tell "feed more bytes" from "a frame was dropped; keep going".
    let mut dec = LogFrameDecoder::new();

    // A partial header is "need more", not a skip.
    dec.feed(&[0x01, 0x00, 0x00]);
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore);

    // A complete header whose payload has not all arrived is also "need
    // more", and its bytes stay buffered.
    dec.feed(&[0x00, 0x00, 0x00, 0x00, 0x03]);
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore);
    assert_eq!(dec.buffered_len(), 8);

    // An unknown stream type (9) with its full payload is "skipped", and its
    // payload is consumed so the next frame decodes right after.
    let mut dec = LogFrameDecoder::new();
    let mut chunk = vec![0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02];
    chunk.extend_from_slice(b"??");
    chunk.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]);
    chunk.extend_from_slice(b"ok");
    dec.feed(&chunk);
    assert_eq!(dec.decode(), DecodeOutcome::Skipped);
    assert_eq!(
        dec.decode(),
        DecodeOutcome::Frame(LogOutput::StdOut { message: bytes::Bytes::from("ok") })
    );
    assert_eq!(dec.decode(), DecodeOutcome::NeedMore);
}
