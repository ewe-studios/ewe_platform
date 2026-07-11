//! Envelope-framing tests (spec-41 F15 / Decision 05): zero-copy decode, frames
//! spanning multiple reads, per-envelope compression, end-stream/trailer frames,
//! and the gRPC timeout codec.

use std::io::{self, Read};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;

use foundation_connectrpc::envelope::{decode_grpc_timeout, encode_grpc_timeout, TimeoutError};
use foundation_connectrpc::{
    Envelope, EnvelopeReader, EnvelopeWriter, GzipCompressor,
};

/// A `Read` source that hands out at most `chunk` bytes per call — models a
/// non-blocking socket delivering a frame across several reads.
struct ChunkedReader {
    data: Vec<u8>,
    pos: usize,
    chunk: usize,
}

impl ChunkedReader {
    fn new(data: Vec<u8>, chunk: usize) -> Self {
        Self { data, pos: 0, chunk }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = &self.data[self.pos..];
        if remaining.is_empty() {
            return Ok(0);
        }
        let n = remaining.len().min(self.chunk).min(buf.len());
        buf[..n].copy_from_slice(&remaining[..n]);
        self.pos += n;
        Ok(n)
    }
}

// ── Envelope encode/decode ────────────────────────────────────────────────────

#[test]
fn envelope_encode_decode_roundtrip() {
    let env = Envelope {
        flags: 0,
        data: Bytes::from_static(b"hello frame"),
    };
    let mut buf = Vec::new();
    env.encode_into(&mut buf);
    assert_eq!(buf[0], 0);
    assert_eq!(&buf[1..5], &(11u32).to_be_bytes());

    let bytes = Bytes::from(buf);
    let (decoded, consumed) = Envelope::decode(&bytes).unwrap();
    assert_eq!(consumed, 5 + 11);
    assert_eq!(decoded.data, Bytes::from_static(b"hello frame"));
}

#[test]
fn envelope_decode_is_zero_copy_slice_of_source() {
    // The payload Bytes must share the source allocation (no memcpy on decode).
    let mut buf = Vec::new();
    Envelope {
        flags: 0,
        data: Bytes::from_static(b"payload-bytes"),
    }
    .encode_into(&mut buf);
    let source = Bytes::from(buf);
    let (env, _) = Envelope::decode(&source).unwrap();
    // A slice of the same allocation points within the source's byte range.
    let src_start = source.as_ptr() as usize;
    let payload_start = env.data.as_ptr() as usize;
    assert!(
        payload_start >= src_start && payload_start < src_start + source.len(),
        "payload must be a zero-copy slice of the source buffer"
    );
}

// ── Reader over a chunked source ──────────────────────────────────────────────

#[test]
fn reader_reassembles_frame_spanning_many_reads() {
    let writer = EnvelopeWriter::new(None, 0, 0);
    let frame = writer.write(Bytes::from_static(b"a message that spans reads")).unwrap();

    // 3 bytes per read forces the 5-byte header + body across many reads.
    let mut reader = EnvelopeReader::new(ChunkedReader::new(frame, 3), None, 0);
    let env = reader.read().unwrap().expect("one frame");
    assert_eq!(env.data, Bytes::from_static(b"a message that spans reads"));
    assert!(reader.read().unwrap().is_none(), "clean end of stream");
}

#[test]
fn reader_yields_two_frames_then_end() {
    let writer = EnvelopeWriter::new(None, 0, 0);
    let mut body = writer.write(Bytes::from_static(b"one")).unwrap();
    body.extend(writer.write(Bytes::from_static(b"two")).unwrap());

    let mut reader = EnvelopeReader::new(ChunkedReader::new(body, 4), None, 0);
    assert_eq!(reader.read().unwrap().unwrap().data, Bytes::from_static(b"one"));
    assert_eq!(reader.read().unwrap().unwrap().data, Bytes::from_static(b"two"));
    assert!(reader.read().unwrap().is_none());
}

#[test]
fn reader_rejects_frame_over_read_max() {
    let writer = EnvelopeWriter::new(None, 0, 0);
    let frame = writer.write(Bytes::from(vec![0u8; 1000])).unwrap();
    let mut reader = EnvelopeReader::new(ChunkedReader::new(frame, 64), None, 100);
    assert!(reader.read().is_err(), "1000-byte frame exceeds read_max 100");
}

// ── Per-envelope compression ──────────────────────────────────────────────────

#[test]
fn compressed_frame_roundtrips_through_reader() {
    let gz: Arc<dyn foundation_connectrpc::Compressor> = Arc::new(GzipCompressor::default());
    let writer = EnvelopeWriter::new(Some(gz.clone()), 0, 0);
    let payload = b"compress me ".repeat(20);
    let frame = writer.write(Bytes::from(payload.clone())).unwrap();
    // Flag bit 0 set (compressed).
    assert_eq!(frame[0] & Envelope::FLAG_COMPRESSED, Envelope::FLAG_COMPRESSED);

    let mut reader = EnvelopeReader::new(ChunkedReader::new(frame, 7), Some(gz), 0);
    let env = reader.read().unwrap().unwrap();
    assert_eq!(env.data, Bytes::from(payload));
}

#[test]
fn compress_min_leaves_small_frames_uncompressed() {
    let gz: Arc<dyn foundation_connectrpc::Compressor> = Arc::new(GzipCompressor::default());
    // compress_min 1000 → a small frame is sent uncompressed.
    let writer = EnvelopeWriter::new(Some(gz), 1000, 0);
    let frame = writer.write(Bytes::from_static(b"tiny")).unwrap();
    assert_eq!(frame[0] & Envelope::FLAG_COMPRESSED, 0, "below compress_min");
}

#[test]
fn send_max_checked_after_compression() {
    let writer = EnvelopeWriter::new(None, 0, 8);
    assert!(writer.write(Bytes::from_static(b"12345678")).is_ok());
    assert!(writer.write(Bytes::from_static(b"123456789")).is_err());
}

// ── End-stream / trailer frames ───────────────────────────────────────────────

#[test]
fn write_end_stream_frame() {
    use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
    let writer = EnvelopeWriter::new(None, 0, 0);
    let mut trailers = SimpleHeaders::new();
    trailers.insert(SimpleHeader::from("x-trailer".to_string()), vec!["v".to_string()]);

    let frame = writer.write_end_stream(None, &trailers).unwrap();
    assert_eq!(frame[0] & Envelope::FLAG_END_STREAM, Envelope::FLAG_END_STREAM);

    // Body is the EndStreamResponse JSON.
    let bytes = Bytes::from(frame);
    let (env, _) = Envelope::decode(&bytes).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&env.data).unwrap();
    assert!(json.get("metadata").is_some());
    assert!(json.get("error").is_none());
}

#[test]
fn write_trailer_frame_grpc_web() {
    use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
    let writer = EnvelopeWriter::new(None, 0, 0);
    let mut trailers = SimpleHeaders::new();
    trailers.insert(SimpleHeader::from("grpc-status".to_string()), vec!["0".to_string()]);

    let frame = writer.write_trailer_frame(&trailers).unwrap();
    assert_eq!(frame[0] & Envelope::FLAG_TRAILER, Envelope::FLAG_TRAILER);
    let bytes = Bytes::from(frame);
    let (env, _) = Envelope::decode(&bytes).unwrap();
    let text = String::from_utf8(env.data.to_vec()).unwrap();
    assert!(text.contains("grpc-status: 0\r\n"));
}

// ── gRPC timeout codec ────────────────────────────────────────────────────────

#[test]
fn grpc_timeout_roundtrip() {
    for d in [
        Duration::from_nanos(100),
        Duration::from_micros(1),
        Duration::from_millis(1),
        Duration::from_secs(1),
        Duration::from_secs(30),
        Duration::from_secs(3600),
    ] {
        let encoded = encode_grpc_timeout(d);
        let decoded = decode_grpc_timeout(&encoded).unwrap();
        assert_eq!(decoded, d, "roundtrip for {encoded}");
    }
}

#[test]
fn grpc_timeout_uses_finest_fitting_unit() {
    // < 100ms fits in nanoseconds (8 digits).
    assert_eq!(encode_grpc_timeout(Duration::from_micros(1)), "1000n");
    // 100ms overflows 8-digit nanos → microseconds.
    assert_eq!(encode_grpc_timeout(Duration::from_millis(100)), "100000u");
}

#[test]
fn grpc_timeout_rejects_over_eight_digits() {
    // 9-digit value.
    assert!(matches!(
        decode_grpc_timeout("123456789n"),
        Err(TimeoutError::TooManyDigits)
    ));
    assert!(matches!(
        decode_grpc_timeout("10X"),
        Err(TimeoutError::InvalidUnit('X'))
    ));
}
