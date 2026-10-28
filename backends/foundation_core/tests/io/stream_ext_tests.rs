//! Tests for `io::stream_ext::ReadBytesExt`: reading from any `Read` into
//! `BytesMut` and pooled buffers.

use std::io::{self, Cursor, Read};
use std::sync::Arc;

use bytes::BytesMut;
use foundation_core::io::buffer_pool::BytesPool;
use foundation_core::io::stream_ext::ReadBytesExt;

/// A reader that hands out at most `chunk` bytes per `read` call, so tests can
/// tell "read up to `max_len`" apart from "read everything available".
struct Trickle<'a> {
    data: &'a [u8],
    chunk: usize,
}

impl Read for Trickle<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.chunk.min(out.len()).min(self.data.len());
        out[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        Ok(n)
    }
}

/// A reader whose every `read` fails.
struct Failing;

impl Read for Failing {
    fn read(&mut self, _out: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::ConnectionReset, "boom"))
    }
}

// -- read_into_bytes --------------------------------------------------------

#[test]
/// WHY: `read_into_bytes` must stop at `max_len` even when more is available
/// WHAT: Only the first `max_len` bytes land in the buffer; the rest stay unread
fn read_into_bytes_reads_at_most_max_len() {
    let mut cursor = Cursor::new(b"hello world".to_vec());
    let mut buf = BytesMut::with_capacity(1024);

    let n = cursor.read_into_bytes(&mut buf, 5).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf[..], b"hello");

    let mut rest = Vec::new();
    cursor.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, b" world");
}

#[test]
/// WHY: `read_into_bytes` appends; it must not overwrite what is already buffered
/// WHAT: Two reads into the same buffer concatenate
fn read_into_bytes_appends_to_existing_contents() {
    let mut cursor = Cursor::new(b"hello world".to_vec());
    let mut buf = BytesMut::from(&b">> "[..]);

    assert_eq!(cursor.read_into_bytes(&mut buf, 6).unwrap(), 6);
    assert_eq!(cursor.read_into_bytes(&mut buf, 64).unwrap(), 5);
    assert_eq!(&buf[..], b">> hello world");
}

#[test]
/// WHY: A buffer with no spare capacity used to return Ok(0) — the EOF signal —
/// even though the reader had data, so callers would stop reading early
/// WHAT: `read_into_bytes` grows the buffer and reads the data
fn read_into_bytes_grows_buffer_without_spare_capacity() {
    let mut cursor = Cursor::new(b"payload".to_vec());

    let mut empty = BytesMut::new();
    assert_eq!(cursor.read_into_bytes(&mut empty, 3).unwrap(), 3);
    assert_eq!(&empty[..], b"pay");

    // Less spare capacity than `max_len`: still reads everything available.
    let mut full = BytesMut::from(&b"ab"[..]);
    assert_eq!(cursor.read_into_bytes(&mut full, 16).unwrap(), 4);
    assert_eq!(&full[..], b"abload");
}

#[test]
/// WHY: A short read must leave only the bytes actually read in the buffer
/// WHAT: The reader returns 2 bytes per call; buffer length follows the count
fn read_into_bytes_short_read_sets_length_to_bytes_read() {
    let mut reader = Trickle {
        data: b"abcdef",
        chunk: 2,
    };
    let mut buf = BytesMut::new();

    assert_eq!(reader.read_into_bytes(&mut buf, 10).unwrap(), 2);
    assert_eq!(&buf[..], b"ab");
    assert_eq!(reader.read_into_bytes(&mut buf, 10).unwrap(), 2);
    assert_eq!(&buf[..], b"abcd");
}

#[test]
/// WHY: 0 is the EOF signal
/// WHAT: An exhausted reader returns 0 and leaves the buffer untouched
fn read_into_bytes_returns_zero_at_eof() {
    let mut cursor = Cursor::new(Vec::<u8>::new());
    let mut buf = BytesMut::from(&b"kept"[..]);

    assert_eq!(cursor.read_into_bytes(&mut buf, 5).unwrap(), 0);
    assert_eq!(&buf[..], b"kept");
}

#[test]
/// WHY: Asking for zero bytes must not consume anything from the reader
/// WHAT: `max_len == 0` returns 0 and the reader still has all its data
fn read_into_bytes_zero_max_len_reads_nothing() {
    let mut cursor = Cursor::new(b"abc".to_vec());
    let mut buf = BytesMut::new();

    assert_eq!(cursor.read_into_bytes(&mut buf, 0).unwrap(), 0);
    assert!(buf.is_empty());
    assert_eq!(cursor.position(), 0);
}

#[test]
/// WHY: A failed read must surface the error and not leave scratch bytes behind
/// WHAT: The error kind is preserved and the buffer keeps its prior contents only
fn read_into_bytes_error_leaves_buffer_unchanged() {
    let mut buf = BytesMut::from(&b"prior"[..]);

    let err = Failing.read_into_bytes(&mut buf, 32).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
    assert_eq!(&buf[..], b"prior");
}

// -- read_exact_into_bytes --------------------------------------------------

#[test]
/// WHY: `read_exact_into_bytes` is used for fixed-size frame headers
/// WHAT: Exactly `len` bytes come back and the reader is positioned after them
fn read_exact_into_bytes_reads_exactly_len() {
    let mut cursor = Cursor::new(b"hello world".to_vec());

    let buf = cursor.read_exact_into_bytes(5).unwrap();
    assert_eq!(&buf[..], b"hello");
    assert_eq!(cursor.position(), 5);
}

#[test]
/// WHY: Fixed-size reads must keep going across short reads
/// WHAT: A reader that yields 1 byte per call still fills the whole length
fn read_exact_into_bytes_spans_short_reads() {
    let mut reader = Trickle {
        data: b"0123456789",
        chunk: 1,
    };

    let buf = reader.read_exact_into_bytes(7).unwrap();
    assert_eq!(&buf[..], b"0123456");
}

#[test]
/// WHY: A truncated stream must not be mistaken for a complete header
/// WHAT: Fewer than `len` bytes is an `UnexpectedEof` error
fn read_exact_into_bytes_errors_on_truncated_input() {
    let mut cursor = Cursor::new(b"hi".to_vec());

    let err = cursor.read_exact_into_bytes(5).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
/// WHY: A zero-length read is trivially satisfied
/// WHAT: Returns an empty buffer without touching the reader
fn read_exact_into_bytes_zero_len_is_empty() {
    let mut cursor = Cursor::new(b"abc".to_vec());

    let buf = cursor.read_exact_into_bytes(0).unwrap();
    assert!(buf.is_empty());
    assert_eq!(cursor.position(), 0);
}

// -- read_byte --------------------------------------------------------------

#[test]
/// WHY: `read_byte` returns one byte at a time and None at EOF
/// WHAT: Walks the input byte by byte, then reports EOF repeatedly
fn read_byte_yields_each_byte_then_none() {
    let mut cursor = Cursor::new(b"hey".to_vec());

    assert_eq!(cursor.read_byte().unwrap(), Some(b'h'));
    assert_eq!(cursor.read_byte().unwrap(), Some(b'e'));
    assert_eq!(cursor.read_byte().unwrap(), Some(b'y'));
    assert_eq!(cursor.read_byte().unwrap(), None);
    assert_eq!(cursor.read_byte().unwrap(), None);
}

#[test]
/// WHY: Reader errors must reach the caller, not be read as EOF
/// WHAT: The underlying error kind is returned
fn read_byte_propagates_errors() {
    let err = Failing.read_byte().unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
}

// -- read_pooled_buffer -----------------------------------------------------

#[test]
/// WHY: `read_pooled_buffer` must hand back only the bytes read
/// WHAT: The pooled buffer's length equals the data read, capped at `max_len`
fn read_pooled_buffer_returns_data_up_to_max_len() {
    let pool = Arc::new(BytesPool::new(1024, 1));
    let mut cursor = Cursor::new(b"hello world".to_vec());

    let first = cursor.read_pooled_buffer(&pool, 5).unwrap().unwrap();
    assert_eq!(&first[..], b"hello");

    let second = cursor.read_pooled_buffer(&pool, 1024).unwrap().unwrap();
    assert_eq!(&second[..], b" world");
}

#[test]
/// WHY: `None` means EOF, so a zero-length request must not return it: the
/// reader is not at its end, nothing was asked for.
/// WHAT: `max_len == 0` gives an empty buffer and leaves the reader untouched
fn read_pooled_buffer_with_zero_max_len_is_not_eof() {
    let pool = Arc::new(BytesPool::new(1024, 1));
    let mut cursor = Cursor::new(b"data".to_vec());

    let empty = cursor
        .read_pooled_buffer(&pool, 0)
        .unwrap()
        .expect("a zero-length read is not end of stream");
    assert!(empty.is_empty());

    let rest = cursor.read_pooled_buffer(&pool, 1024).unwrap().unwrap();
    assert_eq!(&rest[..], b"data");
}

#[test]
/// WHY: None is the EOF signal for pooled reads
/// WHAT: An exhausted reader returns None
fn read_pooled_buffer_returns_none_at_eof() {
    let pool = Arc::new(BytesPool::new(1024, 1));
    let mut cursor = Cursor::new(Vec::<u8>::new());

    assert!(cursor.read_pooled_buffer(&pool, 1024).unwrap().is_none());
}

#[test]
/// WHY: Reader errors must reach the caller, not be read as EOF
/// WHAT: The underlying error kind is returned
fn read_pooled_buffer_propagates_errors() {
    let pool = Arc::new(BytesPool::new(64, 1));

    let err = Failing
        .read_pooled_buffer(&pool, 64)
        .err()
        .expect("a failing reader must return an error");
    assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
}

#[test]
/// WHY: The point of `read_pooled_buffer` is buffer reuse across reads
/// WHAT: Dropping the returned buffer puts it back; the next read is a pool hit
fn read_pooled_buffer_returns_buffer_to_pool_on_drop() {
    let pool = Arc::new(BytesPool::new(64, 1));
    let mut cursor = Cursor::new(b"abcdef".to_vec());

    let buf = cursor.read_pooled_buffer(&pool, 3).unwrap().unwrap();
    assert_eq!(pool.stats().pool_hits, 1);
    assert_eq!(pool.stats().allocations, 0);
    drop(buf);

    let buf = cursor.read_pooled_buffer(&pool, 3).unwrap().unwrap();
    assert_eq!(&buf[..], b"def");
    let stats = pool.stats();
    assert_eq!(stats.pool_hits, 2, "second read reuses the returned buffer");
    assert_eq!(stats.allocations, 0);
}

#[test]
/// WHY: A request larger than the pool's buffers must still be satisfied
/// WHAT: The read allocates a buffer big enough and returns all the data
fn read_pooled_buffer_larger_than_pool_capacity() {
    let pool = Arc::new(BytesPool::new(4, 1));
    let data = vec![7u8; 100];
    let mut cursor = Cursor::new(data.clone());

    let buf = cursor.read_pooled_buffer(&pool, 100).unwrap().unwrap();
    assert_eq!(&buf[..], &data[..]);
    assert_eq!(pool.stats().allocations, 1);
}
