//! Frame checks and `buffer_until` in `shared::http::framing`.
//!
//! Every check is run against the default `FrameLimits` and against a tight set,
//! to show each cap is the configured one and not a built-in constant.

use std::io::Read;

use foundation_core::io::ioutils::ByteBufferPointer;
use foundation_netio::shared::http::framing::{
    buffer_until, chunk_header, header_block, line, sse_event, FrameCheck, FrameKind, FrameLimits,
    FrameTooLarge,
};

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
    assert_eq!(
        header_block(b"Content-Type: a\r\n", &d()),
        FrameCheck::NeedMore
    );
    assert_eq!(
        header_block(b"Content-Type: a\r\nX: b\r", &d()),
        FrameCheck::NeedMore
    );
    assert_eq!(
        header_block(b"Content-Type: a\r\n\r\nbody", &d()),
        FrameCheck::Ready
    );
    assert_eq!(
        header_block(b"\r\n", &d()),
        FrameCheck::Ready,
        "empty block"
    );
    assert_eq!(
        header_block(b"A: b\n\n", &d()),
        FrameCheck::Ready,
        "LF-only"
    );
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
    assert_eq!(
        chunk_header(b"\r\n\r\n", &d()),
        FrameCheck::Ready,
        "no size octets"
    );
}

#[test]
fn line_and_sse_gates() {
    assert_eq!(line(b"abc", &d()), FrameCheck::NeedMore);
    assert_eq!(
        line(b"abc\n", &d()),
        FrameCheck::NeedMore,
        "a lone LF is content"
    );
    assert_eq!(
        line(b"abc\r\n", &d()),
        FrameCheck::NeedMore,
        "the parser reads past the terminator"
    );
    assert_eq!(line(b"abc\r\nd", &d()), FrameCheck::Ready);
    assert_eq!(line(b"abc\n\n\r\n d", &d()), FrameCheck::Ready);
    assert_eq!(sse_event(b"data: x\n", &d()), FrameCheck::NeedMore);
    assert_eq!(sse_event(b"data: x\n\n", &d()), FrameCheck::Ready);
    assert_eq!(sse_event(b"data: x\r\n\r\n", &d()), FrameCheck::Ready);
}

#[test]
fn line_and_sse_honour_their_caps() {
    let limits = tight();
    assert_eq!(
        line(&[b'a'; 33], &limits),
        FrameCheck::TooLarge(FrameKind::Line)
    );
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
