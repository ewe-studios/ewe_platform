//! Non-blocking read semantics of `SharedByteBufferStream` (F47 investigation).
//!
//! WHY: `H2Connection` drives its socket with `read_exact`, and `HttpServer` sets
//! every accepted socket non-blocking. These tests pin down exactly what happens
//! when a read is interrupted by `WouldBlock` partway through — specifically
//! whether bytes already pulled off the stream survive for a retry.
//!
//! The short answer, established below: a `read` that has *some* buffered data
//! swallows `WouldBlock` and returns a short read. Only when the stall persists
//! does `read_exact` return `Err` — and by then it has consumed and discarded
//! everything it read so far. `peek`, by contrast, consumes nothing.
//!
//! WHAT: A `Chunky` reader replaying a scripted sequence of reads so the
//! buffered-reader behaviour is deterministic.

use std::io::{self, Read};
use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::{PeekableReadStream, SharedByteBufferStream};

/// A reader that replays a script of `Ok(bytes)` / `Err(WouldBlock)` steps.
/// Once the script runs out it reports EOF, like a closed socket.
#[derive(Clone)]
struct Chunky {
    steps: Arc<Mutex<Vec<io::Result<Vec<u8>>>>>,
}

impl Chunky {
    fn new(steps: Vec<io::Result<Vec<u8>>>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps)),
        }
    }
}

impl Read for Chunky {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            return Ok(0); // EOF
        }
        match steps.remove(0) {
            Ok(data) => {
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Ok(n)
            }
            Err(e) => Err(e),
        }
    }
}

fn wb() -> io::Result<Vec<u8>> {
    Err(io::Error::new(io::ErrorKind::WouldBlock, "no data yet"))
}

const PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

// ── read() semantics ─────────────────────────────────────────────────────────

/// A `WouldBlock` encountered *after* some bytes were buffered is swallowed:
/// `read` reports the short count instead of failing. This is why a stalled
/// socket does not always surface as an error.
#[test]
fn read_returns_short_count_when_would_block_follows_partial_data() {
    let reader = Chunky::new(vec![Ok(PREFACE[..10].to_vec()), wb(), wb()]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    let n = stream.read(&mut buf).expect("short read, not an error");
    assert_eq!(n, 10, "read should report only what it could buffer");
    assert_eq!(&buf[..10], &PREFACE[..10]);
}

/// With nothing buffered at all, `WouldBlock` propagates and nothing is consumed.
#[test]
fn read_reports_would_block_when_no_data_is_buffered() {
    let reader = Chunky::new(vec![wb(), Ok(PREFACE.to_vec())]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    let err = stream.read(&mut buf).expect_err("expected WouldBlock");
    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);

    // Nothing was consumed: the preface is still fully readable.
    let mut retry = [0u8; 24];
    stream.read_exact(&mut retry).expect("retry");
    assert_eq!(&retry, PREFACE);
}

// ── read_exact() loses data ──────────────────────────────────────────────────

/// `read_exact` is all-or-nothing: a read it cannot satisfy consumes nothing, so
/// the retry sees the message from the start.
///
/// This is the property the whole h2 framing layer rests on. The standard
/// `read_exact` does the opposite — it keeps the bytes it managed to read and
/// then reports failure, leaving the stream advanced mid-message.
#[test]
fn read_exact_interrupted_by_a_persistent_stall_consumes_nothing() {
    // 10 bytes arrive; the socket then stalls, failing the read.
    let reader = Chunky::new(vec![
        Ok(PREFACE[..10].to_vec()),
        wb(),
        // ... later, the rest of the preface shows up, followed by a frame.
        Ok(PREFACE[10..].to_vec()),
        Ok(b"NEXT-FRAME-BYTES-PADDING".to_vec()),
    ]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    let err = stream
        .read_exact(&mut buf)
        .expect_err("interrupted read_exact must fail");
    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);

    // Retry once the data has arrived: the preface must be intact.
    let mut retry = [0u8; 24];
    stream.read_exact(&mut retry).expect("retry read_exact");
    assert_eq!(
        &retry, PREFACE,
        "the failed read_exact must not have consumed the partial bytes"
    );

    // And the following frame is still queued behind it, undisturbed.
    let mut next = [0u8; 24];
    stream.read_exact(&mut next).expect("following frame");
    assert_eq!(&next, b"NEXT-FRAME-BYTES-PADDING");
}

/// The two shortfall cases are distinct, and `read_exact` must not conflate them.
///
/// A *transient* shortfall — the peer is slow — surfaces as `WouldBlock`, which
/// callers understand as "retry later". This is asserted above.
///
/// A *terminal* shortfall — the peer closed after sending 10 of the 24 bytes —
/// must surface as `UnexpectedEof`. Reporting `WouldBlock` here would be a lie:
/// no further bytes are ever coming, so a caller that retries on `WouldBlock`
/// (which is the whole point of that error) would spin forever.
#[test]
fn eof_after_partial_data_reports_unexpected_eof_not_would_block() {
    // 10 bytes, then the peer closes (the script runs out → Ok(0) → EOF).
    let reader = Chunky::new(vec![Ok(PREFACE[..10].to_vec())]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    let err = stream.read_exact(&mut buf).expect_err("stream ended early");
    assert_eq!(
        err.kind(),
        io::ErrorKind::UnexpectedEof,
        "a closed stream must not masquerade as a transient stall"
    );

    // It stays terminal: retrying does not suddenly succeed.
    let mut again = [0u8; 24];
    assert_eq!(
        stream.read_exact(&mut again).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

/// EOF with *nothing* buffered is the same terminal condition, not a stall.
#[test]
fn eof_with_no_data_reports_unexpected_eof() {
    let reader = Chunky::new(vec![]); // immediate EOF
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 8];
    assert_eq!(
        stream.read_exact(&mut buf).unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

/// A caller must not have to `peek` first to get the guarantee above. Reading a
/// frame header and then its payload — the shape every framed protocol uses —
/// leaves nothing half-consumed when the payload has not arrived.
#[test]
fn header_then_payload_reads_leave_nothing_half_consumed() {
    // The 9-byte "header" lands; the payload stalls indefinitely.
    let reader = Chunky::new(vec![Ok(b"HEADER-09".to_vec()), wb(), wb()]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut header = [0u8; 9];
    stream.read_exact(&mut header).expect("header is buffered");
    assert_eq!(&header, b"HEADER-09");

    // The payload is not there; this must not consume anything.
    let mut payload = [0u8; 16];
    let err = stream.read_exact(&mut payload).expect_err("payload not ready");
    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
}

// ── peek() is safe ───────────────────────────────────────────────────────────

/// An interrupted `peek` consumes nothing, so a later `peek` sees the whole
/// preface. This is what makes "pre-buffer, then consume" sound.
#[test]
fn peek_interrupted_by_would_block_consumes_nothing() {
    let reader = Chunky::new(vec![Ok(PREFACE[..10].to_vec()), wb(), Ok(PREFACE[10..].to_vec())]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    assert!(stream.peek(&mut buf).is_err(), "peek could not satisfy 24 bytes");

    let mut buf2 = [0u8; 24];
    let n = stream.peek(&mut buf2).expect("second peek");
    assert_eq!(n, 24);
    assert_eq!(&buf2, PREFACE, "peek must not consume the early bytes");

    let mut consumed = [0u8; 24];
    stream.read_exact(&mut consumed).expect("read after peek");
    assert_eq!(&consumed, PREFACE);
}

/// Once `peek` has buffered the bytes, `read_exact` is served entirely from
/// memory and cannot fail partway — even though the socket is stalled. This is
/// the invariant a frame-atomic h2 reader relies on.
#[test]
fn read_exact_after_successful_peek_is_served_from_the_buffer() {
    let reader = Chunky::new(vec![Ok(PREFACE.to_vec()), wb(), wb()]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut peeked = [0u8; 24];
    assert_eq!(stream.peek(&mut peeked).expect("peek"), 24);

    let mut got = [0u8; 24];
    stream
        .read_exact(&mut got)
        .expect("read_exact must be served from the buffer");
    assert_eq!(&got, PREFACE);
}

/// `peek` on a closed, drained stream must report end-of-stream rather than
/// panicking. A peer that connects and immediately disconnects hits this.
#[test]
fn peek_on_a_closed_empty_stream_reports_eof_without_panicking() {
    let reader = Chunky::new(vec![]); // immediate EOF
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    let n = stream.peek(&mut buf).expect("peek at EOF must not panic or error");
    assert_eq!(n, 0, "no bytes are available on a closed, drained stream");
}

// ── the h2 failure, reproduced ───────────────────────────────────────────────

/// A restarted handshake reads the *next* bytes, not the preface again. This is
/// the concrete origin of `invalid HTTP/2 connection preface`: attempt 1
/// consumes the preface then blocks awaiting a later frame; attempt 2 re-runs
/// step 1 and validates the following frame's bytes as a preface.
#[test]
fn restarting_a_handshake_re_reads_the_following_frame_not_the_preface() {
    let settings_frame = vec![7u8; 24];
    let reader = Chunky::new(vec![Ok(PREFACE.to_vec()), Ok(settings_frame.clone())]);
    let mut stream = SharedByteBufferStream::rwrite(reader);

    let mut buf = [0u8; 24];
    stream.read_exact(&mut buf).expect("preface");
    assert_eq!(&buf, PREFACE);

    // ... attempt 1 fails later (awaiting SETTINGS ACK) and the task parks.
    // Attempt 2 restarts from step 1:
    let mut buf2 = [0u8; 24];
    stream.read_exact(&mut buf2).expect("second read");

    assert_ne!(&buf2, PREFACE, "a restarted handshake cannot see the preface twice");
    assert_eq!(&buf2[..], &settings_frame[..]);
}
