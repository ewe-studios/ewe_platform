//! Frame-completeness checks for reading HTTP/1.1 off a non-blocking socket.
//!
//! WHY: the HTTP/1.1 parsers (`HeaderReader`, `ChunkState`, the line-feed and SSE
//! iterators) consume as they go. They were written for blocking sockets, where a
//! read waits until bytes arrive. On a non-blocking socket a read can return
//! `WouldBlock` halfway through a header block or a chunk header. The parser has
//! already consumed the lines or size octets it read by then, so a retry would
//! resume mid-frame and misparse.
//!
//! WHAT: pure checks over the unconsumed bytes that say whether the next *whole*
//! frame is already buffered ([`FrameCheck`]), plus [`buffer_until`]. That
//! function keeps pulling bytes into a `ByteBufferPointer` until the check passes,
//! the peer hits EOF, or the socket would block.
//!
//! HOW: the caller runs the parser only once the frame is fully buffered, so the
//! parser never touches the socket mid-frame. A `WouldBlock` surfaces from
//! `buffer_until` *before* anything is consumed, so parking and retrying is safe.
//! The checks mirror exactly what the parsers skip and read. When the buffer holds
//! a malformed frame, or more than [`MAX_PREFETCH`] bytes without a terminator,
//! the check reports `Ready` so that the parser runs and produces its own error.

use std::io::Read;

use foundation_core::io::ioutils::ByteBufferPointer;

/// The most bytes buffered while looking for a frame terminator. Past this the
/// parser runs anyway and applies its own size limits, so a peer that never
/// terminates a frame cannot make the client buffer without bound.
pub const MAX_PREFETCH: usize = 16 * 1024 * 1024 + 64 * 1024;

/// Whether the next frame is fully buffered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameCheck {
    /// Parse now: the frame is complete, or the parser must report an error.
    Ready,
    /// The frame is incomplete; read more first.
    NeedMore,
}

/// Which body framing a gate is checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFraming {
    /// `Transfer-Encoding: chunked`.
    Chunked,
    /// One line per item (`\n`-terminated).
    LineFeed,
    /// Server-sent events: one event per blank-line-terminated block.
    Sse,
}

/// Outcome of a chunked-frame check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkCheck {
    /// Whether the next chunk (or the terminal section) is fully buffered.
    pub check: FrameCheck,
    /// `true` when the buffered frame is the last chunk *and* its whole trailer
    /// section. Nothing after it needs gating.
    pub terminal: bool,
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    memchr::memmem::find(haystack, needle)
}

fn over_limit(pending: &[u8]) -> bool {
    pending.len() > MAX_PREFETCH
}

/// Whether a full response header block (through its blank line) is buffered.
#[must_use]
pub fn header_block(pending: &[u8]) -> FrameCheck {
    let complete = pending.starts_with(b"\r\n")
        || pending.starts_with(b"\n")
        || find(pending, b"\r\n\r\n").is_some()
        || find(pending, b"\n\n").is_some();
    if complete || over_limit(pending) {
        FrameCheck::Ready
    } else {
        FrameCheck::NeedMore
    }
}

/// Whether one full line is buffered.
#[must_use]
pub fn line(pending: &[u8]) -> FrameCheck {
    if memchr::memchr(b'\n', pending).is_some() || over_limit(pending) {
        FrameCheck::Ready
    } else {
        FrameCheck::NeedMore
    }
}

/// Whether one full SSE event (through its blank line) is buffered.
#[must_use]
pub fn sse_event(pending: &[u8]) -> FrameCheck {
    if find(pending, b"\n\n").is_some()
        || find(pending, b"\r\n\r\n").is_some()
        || over_limit(pending)
    {
        FrameCheck::Ready
    } else {
        FrameCheck::NeedMore
    }
}

/// Whether the next chunk of a chunked body is fully buffered.
///
/// Mirrors `ChunkState::parse_http_chunk_from_pointer`. That parser skips spaces,
/// then the CRLF left over from the previous chunk's data, then reads hex size
/// octets up to a space, `;`, CR or LF, then any extensions up to the line end,
/// and then reads exactly `size` data bytes. For `size == 0` it returns the last
/// chunk, and the trailer parser reads up to the blank line. So that frame needs
/// the whole trailer section buffered.
#[must_use]
pub fn chunk(pending: &[u8]) -> ChunkCheck {
    let ready = ChunkCheck {
        check: FrameCheck::Ready,
        terminal: false,
    };
    let need_more = ChunkCheck {
        check: if over_limit(pending) {
            FrameCheck::Ready
        } else {
            FrameCheck::NeedMore
        },
        terminal: false,
    };

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

    let digits_start = i;
    let mut size: u64 = 0;
    loop {
        let Some(&b) = pending.get(i) else {
            return need_more;
        };
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            b' ' | b'\r' | b'\n' | b';' => break,
            // Not a size octet: let the parser report it.
            _ => return ready,
        };
        size = match size
            .checked_mul(16)
            .and_then(|s| s.checked_add(u64::from(digit)))
        {
            Some(s) => s,
            // Overflow: the parser rejects it as too large.
            None => return ready,
        };
        i += 1;
    }
    if i == digits_start {
        // No size octets at all: the parser reports `ChunkSizeNotFound`.
        return ready;
    }

    let Some(eol) = memchr::memchr(b'\n', &pending[i..]) else {
        return need_more;
    };
    let after_line = i + eol + 1;

    if size == 0 {
        let rest = &pending[after_line..];
        let complete = rest.starts_with(b"\r\n")
            || rest.starts_with(b"\n")
            || find(rest, b"\r\n\r\n").is_some()
            || find(rest, b"\n\n").is_some();
        return if complete {
            ChunkCheck {
                check: FrameCheck::Ready,
                terminal: true,
            }
        } else {
            need_more
        };
    }

    let Ok(size) = usize::try_from(size) else {
        return ready;
    };
    match after_line.checked_add(size) {
        Some(needed) if pending.len() >= needed => ready,
        // The parser enforces its own maximum chunk size; past the prefetch
        // limit, let it.
        Some(_) => need_more,
        None => ready,
    }
}

/// Result of [`buffer_until`].
#[derive(Debug)]
pub enum BufferOutcome {
    /// The frame is buffered (or the parser must run to report an error/EOF).
    Ready {
        /// Whether the buffered frame was a chunked body's terminal section.
        terminal: bool,
    },
    /// The socket would block before the frame completed. Nothing was consumed.
    WouldBlock,
}

/// Pull bytes into `buffer` until `check` passes on the unconsumed bytes.
///
/// Never consumes. EOF reports `Ready`, so the parser sees the truncation and
/// reports it.
///
/// # Errors
/// Any read error other than `WouldBlock`.
pub fn buffer_until<T: Read>(
    buffer: &mut ByteBufferPointer<T>,
    check: impl Fn(&[u8]) -> ChunkCheck,
) -> std::io::Result<BufferOutcome> {
    loop {
        let verdict = check(&buffer.full_scan()[buffer.data_cursor()..]);
        if verdict.check == FrameCheck::Ready {
            return Ok(BufferOutcome::Ready {
                terminal: verdict.terminal,
            });
        }
        match buffer.fill_up() {
            Ok(0) => return Ok(BufferOutcome::Ready { terminal: false }),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                return Ok(BufferOutcome::WouldBlock);
            }
            Err(err) => return Err(err),
        }
    }
}

/// Adapt a plain [`FrameCheck`] function for [`buffer_until`].
pub fn plain(check: fn(&[u8]) -> FrameCheck) -> impl Fn(&[u8]) -> ChunkCheck {
    move |pending| ChunkCheck {
        check: check(pending),
        terminal: false,
    }
}

/// The gate for one body framing.
#[must_use]
pub fn body_gate(framing: BodyFraming) -> fn(&[u8]) -> ChunkCheck {
    fn line_gate(p: &[u8]) -> ChunkCheck {
        ChunkCheck {
            check: line(p),
            terminal: false,
        }
    }
    fn sse_gate(p: &[u8]) -> ChunkCheck {
        ChunkCheck {
            check: sse_event(p),
            terminal: false,
        }
    }
    match framing {
        BodyFraming::Chunked => chunk,
        BodyFraming::LineFeed => line_gate,
        BodyFraming::Sse => sse_gate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_block_needs_blank_line() {
        assert_eq!(header_block(b"Content-Type: a\r\n"), FrameCheck::NeedMore);
        assert_eq!(
            header_block(b"Content-Type: a\r\nX: b\r"),
            FrameCheck::NeedMore
        );
        assert_eq!(
            header_block(b"Content-Type: a\r\n\r\nbody"),
            FrameCheck::Ready
        );
        assert_eq!(
            header_block(b"\r\n"),
            FrameCheck::Ready,
            "an empty header block"
        );
        assert_eq!(
            header_block(b"A: b\n\n"),
            FrameCheck::Ready,
            "LF-only framing"
        );
    }

    #[test]
    fn chunk_waits_for_size_line_and_data() {
        assert_eq!(chunk(b"").check, FrameCheck::NeedMore);
        assert_eq!(chunk(b"1").check, FrameCheck::NeedMore);
        assert_eq!(chunk(b"5\r").check, FrameCheck::NeedMore);
        assert_eq!(chunk(b"5\r\nhel").check, FrameCheck::NeedMore);
        assert_eq!(chunk(b"5\r\nhello").check, FrameCheck::Ready);
        assert_eq!(chunk(b"\r\n5;ext=1\r\nhello").check, FrameCheck::Ready);
        assert_eq!(
            chunk(b"\r\n1a\r\n").check,
            FrameCheck::NeedMore,
            "0x1a bytes needed"
        );
    }

    #[test]
    fn last_chunk_needs_whole_trailer_section() {
        assert_eq!(chunk(b"0\r\n").check, FrameCheck::NeedMore);
        assert_eq!(chunk(b"0\r\nX-T: 1\r\n").check, FrameCheck::NeedMore);
        let done = chunk(b"0\r\n\r\n");
        assert_eq!(
            done,
            ChunkCheck {
                check: FrameCheck::Ready,
                terminal: true
            }
        );
        let trailers = chunk(b"\r\n0\r\nX-T: 1\r\n\r\n");
        assert_eq!(
            trailers,
            ChunkCheck {
                check: FrameCheck::Ready,
                terminal: true
            }
        );
    }

    #[test]
    fn malformed_chunk_is_left_to_the_parser() {
        assert_eq!(chunk(b"zz\r\n").check, FrameCheck::Ready);
        assert_eq!(
            chunk(b"\r\n\r\n").check,
            FrameCheck::Ready,
            "no size octets"
        );
        assert_eq!(
            chunk(b"ffffffffffffffffff\r\n").check,
            FrameCheck::Ready,
            "overflow"
        );
    }

    #[test]
    fn line_and_sse_gates() {
        assert_eq!(line(b"abc"), FrameCheck::NeedMore);
        assert_eq!(line(b"abc\n"), FrameCheck::Ready);
        assert_eq!(sse_event(b"data: x\n"), FrameCheck::NeedMore);
        assert_eq!(sse_event(b"data: x\n\n"), FrameCheck::Ready);
        assert_eq!(sse_event(b"data: x\r\n\r\n"), FrameCheck::Ready);
    }
}
