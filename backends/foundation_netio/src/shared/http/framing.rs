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

/// Whether one full line is buffered.
#[must_use]
pub fn line(pending: &[u8], limits: &FrameLimits) -> FrameCheck {
    let end = memchr::memchr(b'\n', pending).map(|i| i + 1);
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

/// Make sure at least one unconsumed byte is buffered, reading once if needed.
///
/// Returns the number of unconsumed bytes buffered (`0` at EOF).
///
/// # Errors
/// `WouldBlock` is returned as `Ok(None)`; any other read error is returned.
pub fn some_buffered<T: Read>(buffer: &mut ByteBufferPointer<T>) -> std::io::Result<Option<usize>> {
    if buffer.remaining() > 0 {
        return Ok(Some(buffer.remaining()));
    }
    match buffer.fill_up() {
        Ok(_) => Ok(Some(buffer.remaining())),
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> FrameLimits {
        FrameLimits::default()
    }

    /// Small caps, to show every check honours the configured limit.
    fn tight() -> FrameLimits {
        FrameLimits {
            max_header_block: 64,
            max_chunk_header_line: 16,
            max_line: 32,
            max_sse_event: 48,
        }
    }

    #[test]
    fn header_block_needs_blank_line() {
        assert_eq!(header_block(b"Content-Type: a\r\n", &d()), FrameCheck::NeedMore);
        assert_eq!(
            header_block(b"Content-Type: a\r\nX: b\r", &d()),
            FrameCheck::NeedMore
        );
        assert_eq!(
            header_block(b"Content-Type: a\r\n\r\nbody", &d()),
            FrameCheck::Ready
        );
        assert_eq!(header_block(b"\r\n", &d()), FrameCheck::Ready, "empty block");
        assert_eq!(header_block(b"A: b\n\n", &d()), FrameCheck::Ready, "LF-only");
    }

    #[test]
    fn header_block_honours_its_cap() {
        let limits = tight();
        assert_eq!(
            header_block(&[b'a'; 65], &limits),
            FrameCheck::TooLarge(FrameKind::HeaderBlock)
        );
        let mut late = vec![b'a'; 64];
        late.extend_from_slice(b"\r\n\r\n");
        assert_eq!(
            header_block(&late, &limits),
            FrameCheck::TooLarge(FrameKind::HeaderBlock),
            "a terminator past the cap is still too large"
        );
        let mut at_cap = vec![b'a'; 60];
        at_cap.extend_from_slice(b"\r\n\r\n");
        assert_eq!(header_block(&at_cap, &limits), FrameCheck::Ready);
        assert_eq!(
            header_block(&late, &d()),
            FrameCheck::Ready,
            "the default cap is far larger"
        );
    }

    #[test]
    fn chunk_header_needs_only_the_size_line() {
        assert_eq!(chunk_header(b"", &d()), FrameCheck::NeedMore);
        assert_eq!(chunk_header(b"1", &d()), FrameCheck::NeedMore);
        assert_eq!(chunk_header(b"5\r", &d()), FrameCheck::NeedMore);
        assert_eq!(
            chunk_header(b"5\r\n", &d()),
            FrameCheck::Ready,
            "data is streamed, not gated"
        );
        assert_eq!(chunk_header(b"\r\n5;ext=1\r\nhel", &d()), FrameCheck::Ready);
        assert_eq!(
            chunk_header(b"\r\n4000000\r\n", &d()),
            FrameCheck::Ready,
            "a 64 MiB chunk is ready once its size line is in"
        );
    }

    #[test]
    fn last_chunk_needs_whole_trailer_section() {
        assert_eq!(chunk_header(b"0\r\n", &d()), FrameCheck::NeedMore);
        assert_eq!(chunk_header(b"0\r\nX-T: 1\r\n", &d()), FrameCheck::NeedMore);
        assert_eq!(chunk_header(b"0\r\n\r\n", &d()), FrameCheck::Ready);
        assert_eq!(
            chunk_header(b"\r\n0\r\nX-T: 1\r\n\r\n", &d()),
            FrameCheck::Ready
        );
        assert_eq!(chunk_header(b"00\r\n\r\n", &d()), FrameCheck::Ready);
    }

    #[test]
    fn chunk_header_line_and_trailers_honour_their_caps() {
        let limits = tight();
        let mut long_ext = b"5;".to_vec();
        long_ext.extend([b'x'; 16]);
        assert_eq!(
            chunk_header(&long_ext, &limits),
            FrameCheck::TooLarge(FrameKind::ChunkHeaderLine)
        );
        assert_eq!(
            chunk_header(&[b'0'; 18], &limits),
            FrameCheck::TooLarge(FrameKind::ChunkHeaderLine)
        );
        let mut trailers = b"0\r\n".to_vec();
        trailers.extend([b'x'; 65]);
        assert_eq!(
            chunk_header(&trailers, &limits),
            FrameCheck::TooLarge(FrameKind::TrailerSection)
        );
    }

    #[test]
    fn malformed_chunk_is_left_to_the_parser() {
        assert_eq!(chunk_header(b"zz\r\n", &d()), FrameCheck::Ready);
        assert_eq!(chunk_header(b"\r\n\r\n", &d()), FrameCheck::Ready, "no size octets");
    }

    #[test]
    fn line_and_sse_gates() {
        assert_eq!(line(b"abc", &d()), FrameCheck::NeedMore);
        assert_eq!(line(b"abc\n", &d()), FrameCheck::Ready);
        assert_eq!(sse_event(b"data: x\n", &d()), FrameCheck::NeedMore);
        assert_eq!(sse_event(b"data: x\n\n", &d()), FrameCheck::Ready);
        assert_eq!(sse_event(b"data: x\r\n\r\n", &d()), FrameCheck::Ready);
    }

    #[test]
    fn line_and_sse_honour_their_caps() {
        let limits = tight();
        assert_eq!(line(&[b'a'; 33], &limits), FrameCheck::TooLarge(FrameKind::Line));
        let event = b"data: aaaaaaaaaaaaaaaaaaaa\ndata: bbbbbbbbbbbbbbbbbbbb\n";
        assert_eq!(
            sse_event(event, &limits),
            FrameCheck::TooLarge(FrameKind::SseEvent)
        );
    }

    #[test]
    fn buffer_until_stops_at_the_cap() {
        // A reader that never ends a line: buffer_until must stop at the cap.
        struct Endless;
        impl Read for Endless {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                buf.fill(b'a');
                Ok(buf.len())
            }
        }
        let limits = FrameLimits {
            max_line: 100 * 1024,
            ..FrameLimits::default()
        };
        let mut pointer = ByteBufferPointer::reader(Endless);
        let err = buffer_until(&mut pointer, line, &limits).expect_err("over cap");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(
            FrameTooLarge::in_io(&err),
            Some(FrameTooLarge {
                kind: FrameKind::Line,
                limit: 100 * 1024
            })
        );
        let read_size = foundation_core::io::ioutils::DEFAULT_READ_SIZE;
        assert!(
            pointer.len() <= limits.max_line + read_size,
            "buffered {} bytes for a {}-byte cap",
            pointer.len(),
            limits.max_line
        );
    }
}
