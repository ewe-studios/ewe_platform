//! Bounded frame buffering for reading HTTP/1.1 off a socket.
//!
//! WHY: the HTTP/1.1 parsers (`HeaderReader`, `ChunkState`, the line-feed and SSE
//! parsers) consume as they go. They were written for blocking sockets, where a
//! read waits until bytes arrive. On a non-blocking socket a read can return
//! `WouldBlock` halfway through a header block or a chunk-size line. The parser
//! has already consumed what it read by then, so a retry would resume mid-frame
//! and misparse. On *any* socket, a parser that reads until it sees a delimiter
//! will buffer without limit if the peer never sends one.
//!
//! WHAT: pure checks over the unconsumed bytes that say whether the next *small*
//! frame is fully buffered ([`FrameCheck`]), plus [`buffer_until`]. That function
//! pulls bytes into a `ByteBufferPointer` until the check passes, the peer hits
//! EOF, or the socket would block.
//!
//! Every frame kind has its own configurable cap ([`FrameLimits`]). A frame that
//! exceeds it is an error ([`FrameTooLarge`]), never "let the parser cope". Body
//! *data* is never framed here: a chunked body gates only its chunk-size line,
//! then the chunk iterator streams the chunk's data in pieces of the body
//! extractor's `batch_size`. So no response body is ever held whole in memory.
//!
//! HOW: the caller runs the parser only once the frame is fully buffered, so the
//! parser never touches the socket mid-frame. A `WouldBlock` surfaces from
//! `buffer_until` *before* anything is consumed, so parking and retrying is safe.
//! The checks mirror exactly what the parsers skip and read.

use std::io::Read;

use foundation_core::io::ioutils::ByteBufferPointer;

/// Caps on how much of each frame is buffered before it must be complete.
///
/// None of these limit a body's *data*: chunk data, and the bytes of a
/// `Content-Length` or close-delimited body, stream to the consumer in pieces
/// of the body extractor's `batch_size`. These cap only the frames a parser has
/// to see whole before it can run.
///
/// The defaults follow Go's `net/http`, which Caddy serves with: 1 MiB for a
/// header block (`DefaultMaxHeaderBytes`) and 4 KiB for a chunk-size line
/// (`maxLineLength` in `net/http/internal/chunked.go`). nginx is far stricter,
/// at 4 or 8 KiB for a proxied response header (`proxy_buffer_size`). Set them
/// per client with `ClientConfig::with_frame_limits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLimits {
    /// Largest header block (after the status line), and largest trailer
    /// section after the last chunk.
    pub max_header_block: usize,
    /// Largest chunk-size line: size octets, extensions and line end.
    pub max_chunk_header_line: usize,
    /// Largest single line of a line-delimited body.
    pub max_line: usize,
    /// Largest single server-sent event: all its lines, through the blank line.
    pub max_sse_event: usize,
}

impl Default for FrameLimits {
    fn default() -> Self {
        Self {
            max_header_block: 1024 * 1024,
            max_chunk_header_line: 4 * 1024,
            max_line: 1024 * 1024,
            max_sse_event: 1024 * 1024,
        }
    }
}

impl FrameLimits {
    /// The cap for one frame kind.
    #[must_use]
    pub fn limit(&self, kind: FrameKind) -> usize {
        match kind {
            FrameKind::HeaderBlock | FrameKind::TrailerSection => self.max_header_block,
            FrameKind::ChunkHeaderLine => self.max_chunk_header_line,
            FrameKind::Line => self.max_line,
            FrameKind::SseEvent => self.max_sse_event,
        }
    }
}

/// Whether the next frame is fully buffered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameCheck {
    /// Parse now: the frame is complete, or it is malformed and the parser must
    /// report its own error.
    Ready,
    /// The frame is incomplete; read more first.
    NeedMore,
    /// The frame is longer than its cap. Reading more would buffer without bound.
    TooLarge(FrameKind),
}

/// Which frame went over its cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    HeaderBlock,
    ChunkHeaderLine,
    TrailerSection,
    Line,
    SseEvent,
}

/// A frame exceeded its cap. Carried inside an `io::Error` of kind `InvalidData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTooLarge {
    pub kind: FrameKind,
    pub limit: usize,
}

impl std::fmt::Display for FrameTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} exceeds the {}-byte limit", self.kind, self.limit)
    }
}

impl std::error::Error for FrameTooLarge {}

impl FrameTooLarge {
    /// Whether `err` is (or wraps) a [`FrameTooLarge`].
    #[must_use]
    pub fn in_io(err: &std::io::Error) -> Option<Self> {
        err.get_ref()
            .and_then(|inner| inner.downcast_ref::<Self>())
            .copied()
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    memchr::memmem::find(haystack, needle)
}

/// Index just past the first blank line (`\r\n\r\n` or `\n\n`) in `pending`,
/// treating a leading line end as an empty block.
fn blank_line_end(pending: &[u8]) -> Option<usize> {
    if pending.starts_with(b"\r\n") {
        return Some(2);
    }
    if pending.starts_with(b"\n") {
        return Some(1);
    }
    let crlf = find(pending, b"\r\n\r\n").map(|i| i + 4);
    let lf = find(pending, b"\n\n").map(|i| i + 2);
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// A frame ending at `end` (exclusive) if found, capped at `limit`.
fn capped(end: Option<usize>, pending_len: usize, kind: FrameKind, limit: usize) -> FrameCheck {
    match end {
        Some(end) if end <= limit => FrameCheck::Ready,
        Some(_) => FrameCheck::TooLarge(kind),
        None if pending_len > limit => FrameCheck::TooLarge(kind),
        None => FrameCheck::NeedMore,
    }
}

/// Whether a full response header block (through its blank line) is buffered.
#[must_use]
pub fn header_block(pending: &[u8], limits: &FrameLimits) -> FrameCheck {
    let kind = FrameKind::HeaderBlock;
    capped(blank_line_end(pending), pending.len(), kind, limits.limit(kind))
}

/// Whether one full line is buffered, as `LineFeed::stream_line_feeds` reads it.
///
/// That parser ends a line at `\r\n` or `\n\n` (a lone `\n` is part of the
/// line), then eats the run of spaces, CRs and LFs after it. Those eats read
/// until they see another byte, so the frame is ready only once a byte past
/// that run is buffered (or the peer hits EOF, which `buffer_until` reports).
#[must_use]
pub fn line(pending: &[u8], limits: &FrameLimits) -> FrameCheck {
    let crlf = find(pending, b"\r\n");
    let lflf = find(pending, b"\n\n");
    let terminator = match (crlf, lflf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let end = terminator.and_then(|start| {
        let run = pending[start..]
            .iter()
            .position(|b| !matches!(b, b'\r' | b'\n' | b' '))?;
        Some(start + run)
    });
    capped(end, pending.len(), FrameKind::Line, limits.max_line)
}

/// Whether one full SSE event (through its blank line) is buffered.
#[must_use]
pub fn sse_event(pending: &[u8], limits: &FrameLimits) -> FrameCheck {
    let crlf = find(pending, b"\r\n\r\n").map(|i| i + 4);
    let lf = find(pending, b"\n\n").map(|i| i + 2);
    let end = match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    capped(end, pending.len(), FrameKind::SseEvent, limits.max_sse_event)
}

/// Whether the next chunk-size line of a chunked body is fully buffered. Chunk
/// *data* is not part of the frame: the chunk iterator streams it in pieces.
///
/// Mirrors `ChunkState::parse_http_chunk_from_pointer`. That parser skips spaces,
/// then the CRLF left over from the previous chunk's data, then reads hex size
/// octets up to a space, `;`, CR or LF, then any extensions up to the line end.
/// For `size == 0` it returns the last chunk, and the trailer parser reads up to
/// the blank line, so that frame also needs the whole trailer section buffered
/// (capped at `max_header_block`).
#[must_use]
pub fn chunk_header(pending: &[u8], limits: &FrameLimits) -> FrameCheck {
    let line_cap = limits.max_chunk_header_line;
    let mut i = 0;
    while pending.get(i) == Some(&b' ') {
        i += 1;
    }
    // The parser's three successive optional skips, in the same order.
    if pending[i..].starts_with(b"\r\n") {
        i += 2;
    }
    if pending[i..].starts_with(b"\n\n") {
        i += 2;
    }
    if pending[i..].starts_with(b"\n") {
        i += 1;
    }
    let line_start = i;

    let mut is_zero = true;
    let digits_start = i;
    loop {
        let Some(&b) = pending.get(i) else {
            return capped(
                None,
                pending.len() - line_start,
                FrameKind::ChunkHeaderLine,
                line_cap,
            );
        };
        match b {
            b'0' => {}
            b'1'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => is_zero = false,
            b' ' | b'\r' | b'\n' | b';' => break,
            // Not a size octet: let the parser report it.
            _ => return FrameCheck::Ready,
        }
        i += 1;
        if i - line_start > line_cap {
            return FrameCheck::TooLarge(FrameKind::ChunkHeaderLine);
        }
    }
    if i == digits_start {
        // No size octets at all: the parser reports `ChunkSizeNotFound`.
        return FrameCheck::Ready;
    }

    let eol = memchr::memchr(b'\n', &pending[i..]).map(|e| i + e + 1 - line_start);
    match capped(
        eol,
        pending.len() - line_start,
        FrameKind::ChunkHeaderLine,
        line_cap,
    ) {
        FrameCheck::Ready => {}
        other => return other,
    }
    if !is_zero {
        return FrameCheck::Ready;
    }

    // Last chunk: the trailer section (possibly empty) through its blank line.
    let after_line = line_start + eol.unwrap_or(0);
    let rest = &pending[after_line..];
    let kind = FrameKind::TrailerSection;
    capped(blank_line_end(rest), rest.len(), kind, limits.limit(kind))
}

/// Result of [`buffer_until`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferOutcome {
    /// The frame is buffered (or the parser must run to report an error/EOF).
    Ready,
    /// The socket would block before the frame completed. Nothing was consumed.
    WouldBlock,
}

/// Pull bytes into `buffer` until `check` passes on the unconsumed bytes.
///
/// Never consumes. EOF reports `Ready`, so the parser sees the truncation and
/// reports it.
///
/// # Errors
/// A frame over its cap, as an `InvalidData` error wrapping [`FrameTooLarge`];
/// or any read error other than `WouldBlock`.
pub fn buffer_until<T: Read>(
    buffer: &mut ByteBufferPointer<T>,
    check: fn(&[u8], &FrameLimits) -> FrameCheck,
    limits: &FrameLimits,
) -> std::io::Result<BufferOutcome> {
    loop {
        match check(&buffer.full_scan()[buffer.data_cursor()..], limits) {
            FrameCheck::Ready => return Ok(BufferOutcome::Ready),
            FrameCheck::TooLarge(kind) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    FrameTooLarge {
                        kind,
                        limit: limits.limit(kind),
                    },
                ));
            }
            FrameCheck::NeedMore => {}
        }
        match buffer.fill_up() {
            Ok(0) => return Ok(BufferOutcome::Ready),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                return Ok(BufferOutcome::WouldBlock);
            }
            Err(err) => return Err(err),
        }
    }
}
