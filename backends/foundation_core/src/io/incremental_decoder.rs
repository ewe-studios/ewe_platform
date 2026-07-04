//! `IncrementalDecoder` — the shared resumable frame-decoding seam (Decision 12 §11).
//!
//! WHY: Every frame-oriented protocol we run on a non-blocking fd hits the same
//! wall — a frame can span multiple reads, so a decoder that consumes part of a
//! header/payload and then sees `WouldBlock` (or a short read) must **save its
//! progress and resume**, not error. The Connect/gRPC envelope reader (5-byte
//! prefix + body), the `http2/` 9-byte frame codec, HTTP/3 framing, and the
//! WebSocket decoder all share this requirement, so it is a foundation primitive
//! rather than a per-protocol concern.
//!
//! WHAT: A generic [`IncrementalDecoder`] trait whose [`step`](IncrementalDecoder::step)
//! feeds currently-available bytes and returns [`DecodeStep::Pending`] on a short
//! read (state retained) or [`DecodeStep::Frame`] when a whole frame is ready;
//! errors are reserved for genuine protocol violations. [`AccumulatingBuffer`] is
//! the reusable core each codec is a state machine over — it carries leftover
//! bytes between steps and yields completed payloads as **zero-copy** [`Bytes`].
//! [`read_frame_blocking`] adapts a blocking caller (`loop { step }`-until-frame).
//!
//! HOW: The buffer is a task-owned [`BytesMut`] (it must survive parks, so it is
//! never thread-local and never pooled — Decision 06 read path). Reads land in the
//! buffer's spare capacity; completed frames leave via `split_to().freeze()` (a
//! refcount split, no memcpy on the identity path). Parking composes with Decision
//! 00: a `Pending` on an empty socket lets the driving task `Depends` on the
//! reactor / wake-queue (feature 10) — this primitive stays transport-agnostic.

use std::io::{self, Read};

use bytes::{Buf, Bytes, BytesMut};

/// Default number of bytes each [`AccumulatingBuffer::fill_from`] tries to read.
const DEFAULT_READ_CHUNK: usize = 8 * 1024;

/// Outcome of a single [`IncrementalDecoder::step`].
///
/// `Pending` never means "error" — it means "need more bytes, state retained".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeStep<F> {
    /// Not enough bytes to complete a frame yet; the decoder kept its partial state.
    Pending,
    /// A complete frame was decoded.
    Frame(F),
}

/// Error from incremental decoding — a genuine protocol violation or an I/O
/// failure, **never** a short read (that is [`DecodeStep::Pending`]).
///
/// WHY: Separating "need more bytes" from "the wire is malformed" is what lets one
/// decoder resume across non-blocking reads without spurious errors.
///
/// WHAT: An I/O error surfaced from the source, or a protocol-level violation the
/// decoder detected.
///
/// HOW: `derive_more` supplies `Display`/`From`; `Io` converts from `io::Error`.
#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum DecodeError {
    /// The underlying source returned an I/O error (not `WouldBlock`).
    #[display("i/o error while decoding a frame: {_0}")]
    Io(io::Error),
    /// The bytes on the wire violate the frame format.
    #[from(ignore)]
    #[display("protocol violation while decoding a frame: {_0}")]
    Protocol(Box<str>),
}

impl std::error::Error for DecodeError {}

impl DecodeError {
    /// Build a [`DecodeError::Protocol`] from anything string-like.
    pub fn protocol(message: impl Into<Box<str>>) -> Self {
        DecodeError::Protocol(message.into())
    }
}

/// Drives a frame/record decoder one readable chunk at a time, holding partial
/// state across calls and never erroring on a short read (Decision 12 §11).
pub trait IncrementalDecoder {
    /// The decoded frame type (often [`Bytes`], but codec-specific).
    type Frame;

    /// Feed currently-available bytes (may be empty) and try to decode one frame.
    ///
    /// WHY: Non-blocking sources deliver a frame across several reads; `step` makes
    /// progress with whatever is available and resumes next time.
    ///
    /// WHAT: Reads what it can from `src`, advances the decoder's state machine, and
    /// returns [`DecodeStep::Frame`] on completion or [`DecodeStep::Pending`] when
    /// more bytes are needed (state retained).
    ///
    /// HOW: Typically `buffer.fill_from(src)?` then a state-machine pass over the
    /// accumulated [`AccumulatingBuffer::view`].
    ///
    /// # Errors
    /// [`DecodeError::Io`] on a non-`WouldBlock` source error; [`DecodeError::Protocol`]
    /// on a genuine frame-format violation. A short read is `Pending`, never an error.
    fn step(&mut self, src: &mut impl Read) -> Result<DecodeStep<Self::Frame>, DecodeError>;

    /// Whether the decoder is currently mid-frame (holds buffered/partial bytes).
    ///
    /// WHY: Lets [`read_frame_blocking`] tell a **clean** end-of-stream (`false`)
    /// from a **truncated** frame (`true`) when the source reaches EOF.
    ///
    /// # Panics
    /// Never panics.
    fn has_partial(&self) -> bool;
}

/// Loop [`IncrementalDecoder::step`] against a **blocking** source until one frame
/// is decoded, the source cleanly ends, or a protocol error occurs.
///
/// WHY: Existing blocking callers (today's `decode` / envelope reads) want a simple
/// "give me the next frame" call; this is the backward-compatible wrapper over the
/// resumable `step` seam.
///
/// WHAT: Returns `Ok(Some(frame))` for the next frame, `Ok(None)` on a clean
/// end-of-stream at a frame boundary, or an error.
///
/// HOW: Wraps `src` so a real `read == 0` (EOF) is observed; on EOF with no partial
/// state it returns `Ok(None)`, and with partial state it reports a truncated frame.
///
/// # Errors
/// [`DecodeError::Io`] / [`DecodeError::Protocol`] from `step`, or a truncated-frame
/// [`DecodeError::Protocol`] if the source ends mid-frame.
///
/// # Panics
/// Never panics.
pub fn read_frame_blocking<D, R>(
    decoder: &mut D,
    src: &mut R,
) -> Result<Option<D::Frame>, DecodeError>
where
    D: IncrementalDecoder,
    R: Read,
{
    let mut tracked = EofTrackingReader {
        inner: src,
        hit_eof: false,
    };
    loop {
        match decoder.step(&mut tracked)? {
            DecodeStep::Frame(frame) => return Ok(Some(frame)),
            DecodeStep::Pending => {
                if tracked.hit_eof {
                    return if decoder.has_partial() {
                        Err(DecodeError::protocol("source ended mid-frame (truncated)"))
                    } else {
                        Ok(None)
                    };
                }
                // Blocking source: the next `step` will block on `read` for more.
            }
        }
    }
}

/// A `Read` wrapper that records whether the inner source has hit end-of-stream.
struct EofTrackingReader<'a, R: Read> {
    inner: &'a mut R,
    hit_eof: bool,
}

impl<R: Read> Read for EofTrackingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        // A zero-length request also returns 0 — only a non-empty request reaching
        // 0 is a true EOF.
        if !buf.is_empty() && read == 0 {
            self.hit_eof = true;
        }
        Ok(read)
    }
}

/// The reusable accumulating-buffer core: carries leftover bytes between decode
/// steps, exposes a contiguous view, and yields completed payloads as zero-copy
/// [`Bytes`] (Decision 12 §11).
///
/// WHY: Every incremental codec needs the same "read some, keep the remainder,
/// hand out finished payloads without copying" mechanics.
///
/// WHAT: A task-owned [`BytesMut`] (survives parks — never thread-local, never
/// pooled) plus the per-`fill` read size.
///
/// HOW: `fill_from` reads into the buffer's tail; `split_to` refcount-splits the
/// front out as [`Bytes`]; `advance` discards consumed bytes.
#[derive(Debug)]
pub struct AccumulatingBuffer {
    buf: BytesMut,
    read_chunk: usize,
}

impl Default for AccumulatingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl AccumulatingBuffer {
    /// Create an empty buffer with the default per-read chunk size.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: BytesMut::new(),
            read_chunk: DEFAULT_READ_CHUNK,
        }
    }

    /// Create an empty buffer that pre-reserves `capacity` bytes.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(capacity),
            read_chunk: DEFAULT_READ_CHUNK,
        }
    }

    /// Set how many bytes each [`fill_from`](Self::fill_from) attempts to read
    /// (clamped to a minimum of 1). Returns `self` for chaining.
    #[must_use]
    pub fn with_read_chunk(mut self, read_chunk: usize) -> Self {
        self.read_chunk = read_chunk.max(1);
        self
    }

    /// Number of buffered (undecoded) bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether no bytes are currently buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Contiguous view of the accumulated bytes (for the decoder's state machine).
    #[must_use]
    pub fn view(&self) -> &[u8] {
        &self.buf
    }

    /// Read one chunk of currently-available bytes from `src` into the buffer.
    ///
    /// WHY: The "feed currently-available bytes" step — one best-effort read.
    ///
    /// WHAT: Returns the number of new bytes appended. `Ok(0)` means either EOF or
    /// (on a non-blocking source) no data is available right now — `WouldBlock` is
    /// mapped to `0` so `step` can return `Pending` cleanly.
    ///
    /// HOW: Grows the buffer by `read_chunk`, reads directly into that spare region
    /// (no temp-buffer copy), then truncates to what was actually read.
    ///
    /// # Errors
    /// Any source error other than `WouldBlock`.
    ///
    /// # Panics
    /// Never panics.
    pub fn fill_from(&mut self, src: &mut impl Read) -> io::Result<usize> {
        let start = self.buf.len();
        // Zero-extend the tail so we have an initialized `&mut [u8]` to read into —
        // one memset, no separate temp buffer / second data copy.
        self.buf.resize(start + self.read_chunk, 0);
        match src.read(&mut self.buf[start..]) {
            Ok(read) => {
                self.buf.truncate(start + read);
                Ok(read)
            }
            Err(err) => {
                self.buf.truncate(start);
                if err.kind() == io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    Err(err)
                }
            }
        }
    }

    /// Split the first `n` bytes off the front as a zero-copy [`Bytes`], advancing
    /// the buffer past them.
    ///
    /// # Panics
    /// Panics if `n > len()`.
    #[must_use = "the split-off payload is the decoded frame"]
    pub fn split_to(&mut self, n: usize) -> Bytes {
        assert!(
            n <= self.buf.len(),
            "split_to({n}) exceeds buffered length {}",
            self.buf.len()
        );
        self.buf.split_to(n).freeze()
    }

    /// Discard the first `n` bytes (e.g. a consumed frame header).
    ///
    /// # Panics
    /// Panics if `n > len()`.
    pub fn advance(&mut self, n: usize) {
        assert!(
            n <= self.buf.len(),
            "advance({n}) exceeds buffered length {}",
            self.buf.len()
        );
        self.buf.advance(n);
    }

    /// Drop all buffered bytes.
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}
