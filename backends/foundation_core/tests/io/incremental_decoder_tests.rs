//! Feature 04 (Decision 12 §11): the shared `IncrementalDecoder` seam.
//!
//! These tests use a sample length-prefixed decoder (`[u32 BE len][payload]`) built
//! on `AccumulatingBuffer` — the same shape the real envelope/http2/WS codecs will
//! take — to prove: a frame spanning many short reads decodes without error and
//! with state retained; completed payloads come out as zero-copy `Bytes`; and the
//! blocking wrapper handles clean EOS vs a truncated frame.

use std::io::{self, Cursor, Read};

use bytes::Bytes;
use foundation_core::io::{
    read_frame_blocking, AccumulatingBuffer, DecodeError, DecodeStep, IncrementalDecoder,
};

/// A sample decoder: frames are `[u32 big-endian length][payload bytes]`.
struct LenPrefixDecoder {
    buf: AccumulatingBuffer,
}

impl LenPrefixDecoder {
    fn new() -> Self {
        Self {
            buf: AccumulatingBuffer::new(),
        }
    }
}

impl IncrementalDecoder for LenPrefixDecoder {
    type Frame = Bytes;

    fn step(&mut self, src: &mut impl Read) -> Result<DecodeStep<Bytes>, DecodeError> {
        // Feed whatever is currently available.
        self.buf.fill_from(src)?;

        // Need the 4-byte length prefix first.
        if self.buf.len() < 4 {
            return Ok(DecodeStep::Pending);
        }
        let len = u32::from_be_bytes(self.buf.view()[..4].try_into().unwrap()) as usize;

        // Then the whole payload.
        if self.buf.len() < 4 + len {
            return Ok(DecodeStep::Pending);
        }

        self.buf.advance(4); // drop the consumed prefix
        let payload = self.buf.split_to(len); // zero-copy Bytes
        Ok(DecodeStep::Frame(payload))
    }

    fn has_partial(&self) -> bool {
        !self.buf.is_empty()
    }
}

/// A `Read` that yields at most `chunk` bytes per call — simulates short reads on a
/// non-blocking socket, then blocks-style EOF once drained.
struct ChunkedReader {
    data: Cursor<Vec<u8>>,
    chunk: usize,
}

impl ChunkedReader {
    fn new(data: Vec<u8>, chunk: usize) -> Self {
        Self {
            data: Cursor::new(data),
            chunk: chunk.max(1),
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let cap = buf.len().min(self.chunk);
        self.data.read(&mut buf[..cap])
    }
}

fn framed(payload: &[u8]) -> Vec<u8> {
    let mut out = (payload.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

/// WHY: A frame that arrives across many short reads must decode without error and
///      keep its partial state between reads (Decision 12 §11 acceptance).
#[test]
fn frame_spanning_multiple_reads_decodes_without_error() {
    let payload = b"hello, incremental world";
    let mut src = ChunkedReader::new(framed(payload), 1); // ONE byte per read
    let mut decoder = LenPrefixDecoder::new();

    // Every read before completion yields Pending, never an error, state retained.
    let mut pending_seen = 0;
    let frame = loop {
        match decoder.step(&mut src).expect("no error on short reads") {
            DecodeStep::Frame(frame) => break frame,
            DecodeStep::Pending => {
                pending_seen += 1;
                assert!(
                    decoder.has_partial() || pending_seen == 1,
                    "partial state must be retained once bytes have arrived"
                );
                assert!(pending_seen < 10_000, "should not loop forever");
            }
        }
    };

    assert_eq!(frame.as_ref(), payload);
    assert!(
        pending_seen >= payload.len(),
        "a 1-byte-per-read source must have parked (Pending) many times: {pending_seen}"
    );
}

/// WHY: A short read partway through the payload must retain state and resume.
#[test]
fn short_read_yields_pending_with_state_retained() {
    let payload = b"partial-state";
    let full = framed(payload);

    let mut decoder = LenPrefixDecoder::new();

    // Feed only the first 6 bytes (length prefix + 2 payload bytes).
    let mut first = Cursor::new(full[..6].to_vec());
    assert_eq!(
        decoder.step(&mut first).unwrap(),
        DecodeStep::Pending,
        "incomplete payload -> Pending"
    );
    assert!(decoder.has_partial(), "decoder retains the partial frame");

    // Feed the rest; the frame completes using the retained state.
    let mut rest = Cursor::new(full[6..].to_vec());
    match decoder.step(&mut rest).unwrap() {
        DecodeStep::Frame(frame) => assert_eq!(frame.as_ref(), payload),
        DecodeStep::Pending => panic!("frame should complete once the rest arrives"),
    }
    assert!(!decoder.has_partial(), "no partial state after the frame completes");
}

/// WHY: Completed payloads must be zero-copy `Bytes` slices of the accumulated
///      buffer — no per-frame memcpy on the identity path.
#[test]
fn completed_payload_is_a_zero_copy_bytes_slice() {
    // Two frames delivered in one read; both come out as independent Bytes that
    // share the buffer's allocation (split, not copied).
    let mut data = framed(b"first");
    data.extend_from_slice(&framed(b"second"));
    let mut src = Cursor::new(data);
    let mut decoder = LenPrefixDecoder::new();

    let f1 = match decoder.step(&mut src).unwrap() {
        DecodeStep::Frame(f) => f,
        DecodeStep::Pending => panic!("first frame should be ready"),
    };
    // `Bytes::split`-derived slices are refcounted views, not fresh allocations.
    let f2 = match decoder.step(&mut src).unwrap() {
        DecodeStep::Frame(f) => f,
        DecodeStep::Pending => panic!("second frame should be ready from the same buffer"),
    };

    assert_eq!(f1.as_ref(), b"first");
    assert_eq!(f2.as_ref(), b"second");
    // A cheap clone of a Bytes shares storage — proves these are views, not copies.
    let f1_clone = f1.clone();
    assert_eq!(f1_clone.as_ptr(), f1.as_ptr(), "clone shares the same backing storage");
}

/// WHY: The blocking wrapper must return a full frame, then clean EOS as `Ok(None)`.
#[test]
fn blocking_wrapper_reads_frame_then_clean_eos() {
    let mut data = framed(b"one");
    data.extend_from_slice(&framed(b"two"));
    let mut src = Cursor::new(data);
    let mut decoder = LenPrefixDecoder::new();

    let one = read_frame_blocking(&mut decoder, &mut src).unwrap();
    assert_eq!(one.as_deref(), Some(&b"one"[..]));

    let two = read_frame_blocking(&mut decoder, &mut src).unwrap();
    assert_eq!(two.as_deref(), Some(&b"two"[..]));

    // Nothing left, at a clean frame boundary -> Ok(None).
    let end = read_frame_blocking(&mut decoder, &mut src).unwrap();
    assert_eq!(end, None, "clean end-of-stream is Ok(None)");
}

/// WHY: A source that ends mid-frame must be reported as a truncation error, not a
///      silent clean end.
#[test]
fn blocking_wrapper_reports_truncated_frame() {
    let full = framed(b"truncated-payload");
    // Drop the last 3 bytes so the frame can never complete.
    let mut src = Cursor::new(full[..full.len() - 3].to_vec());
    let mut decoder = LenPrefixDecoder::new();

    match read_frame_blocking(&mut decoder, &mut src) {
        Err(DecodeError::Protocol(_)) => {}
        other => panic!("expected a truncated-frame protocol error, got {other:?}"),
    }
}

/// WHY: A genuine I/O error (not `WouldBlock`) must surface as `DecodeError::Io`.
#[test]
fn io_error_surfaces_as_decode_error() {
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "boom"))
        }
    }

    let mut decoder = LenPrefixDecoder::new();
    match decoder.step(&mut BrokenReader) {
        Err(DecodeError::Io(e)) => assert_eq!(e.kind(), io::ErrorKind::BrokenPipe),
        other => panic!("expected DecodeError::Io, got {other:?}"),
    }
}

/// WHY: `WouldBlock` is "no data right now", not an error — it must yield `Pending`.
#[test]
fn would_block_is_pending_not_error() {
    struct WouldBlockReader(bool);
    impl Read for WouldBlockReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                // First call: deliver a full small frame.
                self.0 = false;
                let frame = framed(b"x");
                buf[..frame.len()].copy_from_slice(&frame);
                Ok(frame.len())
            } else {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "try later"))
            }
        }
    }

    let mut decoder = LenPrefixDecoder::new();
    let mut src = WouldBlockReader(true);

    // First step reads the whole frame.
    assert_eq!(
        decoder.step(&mut src).unwrap(),
        DecodeStep::Frame(Bytes::from_static(b"x"))
    );
    // Next step: source WouldBlocks -> Pending, no error.
    assert_eq!(decoder.step(&mut src).unwrap(), DecodeStep::Pending);
}
