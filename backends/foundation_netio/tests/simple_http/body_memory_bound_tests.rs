//! Response bodies never buffer whole, and every frame a parser must see whole
//! is capped by the configured `FrameLimits`.
//!
//! WHY: a server controls how big a chunk, a line, an event or a header block
//! is. Before these caps, a chunk was read whole (up to 16 MiB) and a line or
//! SSE event that never ended grew the buffer without bound.
//!
//! HOW: each test feeds a parser from a `Read` that produces bytes on demand and
//! counts them, then checks two things: data reaches the consumer long before
//! the producer is done, and the parser's buffer never holds more than a small
//! bound. Caps are set per test to prove they are configurable.

use std::cell::Cell;
use std::io::Read;
use std::rc::Rc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::shared::http::framing::{FrameKind, FrameLimits, FrameTooLarge};
use foundation_netio::shared::http::{
    ChunkedData, HttpReaderError, HttpResponseReader, IncomingResponseParts, SimpleHeaders,
    SimpleHttpBody, SimpleHttpChunkIterator, SimpleLineFeedIterator, SimpleSseIterator,
};

/// Produces `prefix`, then `body_len` bytes of `fill`, then `suffix`, counting
/// how many bytes it has handed out.
struct Generated {
    prefix: Vec<u8>,
    body_len: usize,
    fill: u8,
    suffix: Vec<u8>,
    produced: Rc<Cell<usize>>,
}

impl Generated {
    fn new(prefix: &[u8], body_len: usize, fill: u8, suffix: &[u8]) -> (Self, Rc<Cell<usize>>) {
        let produced = Rc::new(Cell::new(0));
        (
            Self {
                prefix: prefix.to_vec(),
                body_len,
                fill,
                suffix: suffix.to_vec(),
                produced: produced.clone(),
            },
            produced,
        )
    }

    /// A body that never ends.
    fn endless(prefix: &[u8], fill: u8) -> (Self, Rc<Cell<usize>>) {
        Self::new(prefix, usize::MAX, fill, b"")
    }
}

impl Read for Generated {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let at = self.produced.get();
        let body_end = self.prefix.len().saturating_add(self.body_len);
        let n = if at < self.prefix.len() {
            let n = (self.prefix.len() - at).min(buf.len());
            buf[..n].copy_from_slice(&self.prefix[at..at + n]);
            n
        } else if at < body_end {
            let n = (body_end - at).min(buf.len());
            buf[..n].fill(self.fill);
            n
        } else {
            let off = at - body_end;
            let n = self.suffix.len().saturating_sub(off).min(buf.len());
            buf[..n].copy_from_slice(&self.suffix[off..off + n]);
            n
        };
        self.produced.set(at + n);
        Ok(n)
    }
}

fn buffered<T: Read>(stream: &SharedByteBufferStream<T>) -> usize {
    stream.do_once(|pointer| pointer.len())
}

fn too_large(err: &(dyn std::error::Error + 'static)) -> Option<FrameTooLarge> {
    match err.downcast_ref::<HttpReaderError>() {
        Some(HttpReaderError::FrameTooLarge(t)) => Some(*t),
        _ => None,
    }
}

#[test]
fn a_64_mib_chunk_streams_in_batch_size_pieces() {
    const CHUNK: usize = 64 * 1024 * 1024;
    const PIECE: usize = 8192;
    let (reader, produced) =
        Generated::new(format!("{CHUNK:x}\r\n").as_bytes(), CHUNK, b'x', b"\r\n0\r\n\r\n");
    let stream = SharedByteBufferStream::ref_cell(reader);
    let mut iter = SimpleHttpChunkIterator::new(vec![], SimpleHeaders::new(), stream.clone())
        .with_limits(PIECE, FrameLimits::default());

    let mut total = 0usize;
    let mut pieces = 0usize;
    let mut first_piece_at = None;
    let mut peak = 0usize;
    for item in &mut iter {
        match item.expect("chunk iterator") {
            ChunkedData::Data(bytes, _) => {
                assert!(bytes.len() <= PIECE, "piece of {} bytes", bytes.len());
                assert!(bytes.iter().all(|b| *b == b'x'));
                first_piece_at.get_or_insert(produced.get());
                total += bytes.len();
                pieces += 1;
                peak = peak.max(buffered(&stream));
            }
            ChunkedData::DataEnded => break,
            ChunkedData::Trailers(_) => {}
        }
    }

    assert_eq!(total, CHUNK);
    assert!(pieces >= CHUNK / PIECE);
    let first = first_piece_at.expect("got data");
    assert!(
        first < 1024 * 1024,
        "first piece came after {first} bytes were read: the chunk was buffered"
    );
    assert!(
        peak < 256 * 1024,
        "the parser held {peak} bytes at once for a {CHUNK}-byte chunk"
    );
}

#[test]
fn an_oversized_chunk_size_line_errors_at_its_cap() {
    let limits = FrameLimits {
        max_chunk_header_line: 64,
        ..FrameLimits::default()
    };
    // `5;` then extensions that never end.
    let (reader, produced) = Generated::endless(b"5;", b'e');
    let stream = SharedByteBufferStream::ref_cell(reader);
    let mut iter = SimpleHttpChunkIterator::new(vec![], SimpleHeaders::new(), stream.clone())
        .with_limits(8192, limits);

    let err = iter.next().expect("an item").expect_err("over cap");
    assert_eq!(
        too_large(&*err),
        Some(FrameTooLarge {
            kind: FrameKind::ChunkHeaderLine,
            limit: 64
        })
    );
    assert!(produced.get() < 256 * 1024, "read {} bytes", produced.get());
}

#[test]
fn an_endless_line_errors_at_the_configured_cap() {
    let limits = FrameLimits {
        max_line: 4096,
        ..FrameLimits::default()
    };
    let (reader, produced) = Generated::endless(b"", b'l');
    let stream = SharedByteBufferStream::ref_cell(reader);
    let mut iter = SimpleLineFeedIterator::new(SimpleHeaders::new(), stream.clone()).with_limits(limits);

    let err = iter.next().expect("an item").expect_err("over cap");
    assert_eq!(
        too_large(&*err),
        Some(FrameTooLarge {
            kind: FrameKind::Line,
            limit: 4096
        })
    );
    assert!(buffered(&stream) < 256 * 1024);
    assert!(produced.get() < 256 * 1024, "read {} bytes", produced.get());
}

#[test]
fn lines_under_the_cap_still_parse() {
    let limits = FrameLimits {
        max_line: 16,
        ..FrameLimits::default()
    };
    let (reader, _) = Generated::new(b"one\ntwo\n", 0, b'-', b"");
    let stream = SharedByteBufferStream::ref_cell(reader);
    let lines: Vec<_> = SimpleLineFeedIterator::new(SimpleHeaders::new(), stream)
        .with_limits(limits)
        .take(2)
        .map(|r| format!("{:?}", r.expect("line")))
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("one") && lines[1].contains("two"), "{lines:?}");
}

#[test]
fn an_endless_sse_event_errors_at_the_configured_cap() {
    let limits = FrameLimits {
        max_sse_event: 4096,
        ..FrameLimits::default()
    };
    // One `data:` line that never ends, so the event never ends either.
    let (reader, produced) = Generated::endless(b"data: ", b's');
    let stream = SharedByteBufferStream::ref_cell(reader);
    let mut iter = SimpleSseIterator::new(SimpleHeaders::new(), stream.clone()).with_limits(limits);

    let err = iter.next().expect("an item").expect_err("over cap");
    assert_eq!(
        too_large(&*err),
        Some(FrameTooLarge {
            kind: FrameKind::SseEvent,
            limit: 4096
        })
    );
    assert!(produced.get() < 256 * 1024, "read {} bytes", produced.get());
}

#[test]
fn an_endless_header_block_errors_at_the_configured_cap() {
    let limits = FrameLimits {
        max_header_block: 8 * 1024,
        ..FrameLimits::default()
    };
    let (reader, produced) = Generated::endless(b"HTTP/1.1 200 OK\r\nX-Big: ", b'h');
    let stream = SharedByteBufferStream::ref_cell(reader);
    let mut reader = HttpResponseReader::new(
        stream,
        SimpleHttpBody::default().with_frame_limits(limits),
    );

    let intro = reader.next().expect("intro").expect("status line parses");
    assert!(matches!(intro, IncomingResponseParts::Intro(..)), "{intro:?}");
    match reader.next().expect("headers item") {
        Err(HttpReaderError::FrameTooLarge(t)) => assert_eq!(
            t,
            FrameTooLarge {
                kind: FrameKind::HeaderBlock,
                limit: 8 * 1024
            }
        ),
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
    assert!(produced.get() < 256 * 1024, "read {} bytes", produced.get());
}

/// A reader that returns `WouldBlock` once at each listed offset, like a
/// non-blocking socket whose bytes arrive in bursts.
struct Bursty {
    data: Vec<u8>,
    at: usize,
    stalls: Vec<usize>,
}

impl Read for Bursty {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(pos) = self.stalls.iter().position(|s| *s == self.at) {
            self.stalls.remove(pos);
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        let next_stall = self
            .stalls
            .iter()
            .copied()
            .filter(|s| *s > self.at)
            .min()
            .unwrap_or(self.data.len());
        let n = (next_stall - self.at).min(buf.len());
        buf[..n].copy_from_slice(&self.data[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

#[test]
fn would_block_mid_frame_resumes_without_losing_bytes() {
    let body = b"hello, world";
    let wire = format!("{:x};ext=1\r\n", body.len());
    let mut data = wire.clone().into_bytes();
    data.extend_from_slice(body);
    data.extend_from_slice(b"\r\n0\r\nX-T: 1\r\n\r\n");
    // Stall inside the size line, inside the data, and inside the trailers.
    let stalls = vec![2, wire.len() + 3, data.len() - 4];
    let stream = SharedByteBufferStream::ref_cell(Bursty { data, at: 0, stalls });
    let mut iter =
        SimpleHttpChunkIterator::new(vec![], SimpleHeaders::new(), stream).with_limits(4, FrameLimits::default());

    let mut got = Vec::new();
    let mut would_blocks = 0;
    let mut ended = false;
    let mut trailers = None;
    for _ in 0..100 {
        match iter.next() {
            Some(Ok(ChunkedData::Data(bytes, _))) => got.extend(bytes),
            Some(Ok(ChunkedData::DataEnded)) => ended = true,
            Some(Ok(ChunkedData::Trailers(t))) => trailers = Some(t),
            Some(Err(err)) => {
                let is_wb = matches!(
                    err.downcast_ref::<HttpReaderError>(),
                    Some(HttpReaderError::WouldBlock)
                );
                assert!(is_wb, "unexpected error: {err}");
                would_blocks += 1;
            }
            None => break,
        }
    }
    assert_eq!(got, body);
    assert!(ended);
    assert!(would_blocks >= 2, "only {would_blocks} would-blocks surfaced");
    let trailers = trailers.expect("trailers parsed");
    assert_eq!(trailers.len(), 1, "{trailers:?}");
}
