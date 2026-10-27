//! Read-path tests for `ByteBufferPointer` (`read_size`, `read_all`, the `Read`
//! and `PeekableReadStream` impls) and `SharedByteBufferStream`, which reads
//! through it.
//!
//! WHY: `read_size` used to copy from the peek cursor rather than the data
//! cursor, then `skip()` everything up to the new peek cursor. Any bytes that
//! had been peeked over (`nextby`, `forward_by`, the `PeekableReadStream::peek`
//! impl, which itself advanced the peek cursor) were dropped without ever
//! being returned. `read_all` had the same mistake, and the dead
//! `LessThanRequested` arm copied from the start of the buffer.
//!
//! These tests pin down that every read starts at the first unconsumed byte,
//! consumes exactly what it returns, and keeps working across buffer refills
//! and at EOF.

use std::collections::VecDeque;
use std::io::{self, Cursor, Read};
use std::sync::{Arc, Mutex};

use foundation_core::io::ioutils::{
    ByteBufferPointer, PeekState, PeekableReadStream, SharedByteBufferStream,
};

fn sample(len: usize) -> Vec<u8> {
    (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect()
}

fn pointer(pull_amount: usize, data: &[u8]) -> ByteBufferPointer<Cursor<Vec<u8>>> {
    ByteBufferPointer::from_reader(pull_amount, Cursor::new(data.to_vec()))
}

/// A reader that replays a script of `Ok(bytes)` / `Err(kind)` steps, then
/// reports EOF. Each `Ok` step is returned by one `read` call.
#[derive(Clone)]
struct Scripted {
    steps: Arc<Mutex<VecDeque<io::Result<Vec<u8>>>>>,
}

impl Scripted {
    fn new(steps: Vec<io::Result<Vec<u8>>>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps.into())),
        }
    }
}

impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut steps = self.steps.lock().unwrap();
        match steps.pop_front() {
            None => Ok(0),
            Some(Err(err)) => Err(err),
            Some(Ok(mut bytes)) => {
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                if n < bytes.len() {
                    // Keep what did not fit for the next call.
                    steps.push_front(Ok(bytes.split_off(n)));
                }
                Ok(n)
            }
        }
    }
}

#[test]
fn sequential_read_size_calls_return_consecutive_bytes() {
    let data = sample(100);
    // A pull amount that does not divide the read size, so reads straddle
    // refills and buffer compaction.
    let mut ptr = pointer(7, &data);

    let mut collected = Vec::new();
    let mut buf = [0u8; 10];
    loop {
        let n = ptr.read_size(&mut buf).expect("read should succeed");
        println!("read_size -> {n} bytes: {:?}", &buf[..n]);
        if n == 0 {
            break;
        }
        assert_eq!(n, 10, "every read but EOF should fill the buffer");
        collected.extend_from_slice(&buf[..n]);
    }

    assert_eq!(
        collected, data,
        "bytes must be consecutive and never repeat"
    );
}

#[test]
fn short_buf_then_more_reads_continue_where_the_last_left_off() {
    let data = b"abcdefghijklmnopqrstuvwxyz";
    let mut ptr = pointer(16, data);

    let mut small = [0u8; 3];
    assert_eq!(ptr.read_size(&mut small).unwrap(), 3);
    assert_eq!(&small, b"abc");

    // Exactly the three returned bytes are consumed, nothing more.
    assert_eq!(ptr.data_cursor(), ptr.peek_cursor());

    let mut medium = [0u8; 5];
    assert_eq!(ptr.read_size(&mut medium).unwrap(), 5);
    assert_eq!(&medium, b"defgh");

    let mut large = [0u8; 64];
    let n = ptr.read_size(&mut large).unwrap();
    println!("tail read -> {:?}", std::str::from_utf8(&large[..n]));
    assert_eq!(&large[..n], b"ijklmnopqrstuvwxyz");
}

#[test]
fn less_than_requested_after_partial_consume_returns_the_unread_tail() {
    let data = b"abcdefghij";
    let mut ptr = pointer(4, data);

    let mut first = [0u8; 3];
    assert_eq!(ptr.read_size(&mut first).unwrap(), 3);
    assert_eq!(&first, b"abc");

    // Ask for more than the stream has left.
    let mut rest = [0u8; 20];
    let n = ptr.read_size(&mut rest).unwrap();
    println!(
        "less-than-requested read -> {:?}",
        std::str::from_utf8(&rest[..n])
    );
    assert_eq!(n, 7);
    assert_eq!(&rest[..n], b"defghij");

    assert_eq!(ptr.read_size(&mut rest).unwrap(), 0);
}

#[test]
fn less_than_requested_with_a_trickling_reader() {
    // The reader hands back a few bytes per call, so the buffer is compacted
    // after the first read consumed some of it.
    let reader = Scripted::new(vec![
        Ok(b"hello".to_vec()),
        Ok(b" wor".to_vec()),
        Ok(b"ld".to_vec()),
    ]);
    let mut ptr = ByteBufferPointer::from_reader(5, reader);

    let mut first = [0u8; 2];
    assert_eq!(ptr.read_size(&mut first).unwrap(), 2);
    assert_eq!(&first, b"he");

    let mut rest = [0u8; 32];
    let n = ptr.read_size(&mut rest).unwrap();
    println!("trickled read -> {:?}", std::str::from_utf8(&rest[..n]));
    assert_eq!(&rest[..n], b"llo world");
}

#[test]
fn eof_returns_zero_and_keeps_returning_zero() {
    let mut empty = pointer(8, b"");
    let mut buf = [0u8; 4];
    assert_eq!(empty.read_size(&mut buf).unwrap(), 0);
    assert_eq!(empty.read_size(&mut buf).unwrap(), 0);

    let mut ptr = pointer(8, b"xyz");
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 3);
    assert_eq!(&buf[..3], b"xyz");
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 0);
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 0);
}

#[test]
fn zero_length_buf_reads_nothing_and_consumes_nothing() {
    let mut ptr = pointer(8, b"abc");
    assert_eq!(ptr.read_size(&mut []).unwrap(), 0);

    let mut buf = [0u8; 3];
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 3);
    assert_eq!(&buf, b"abc");
}

#[test]
fn read_after_nextby_returns_the_peeked_bytes() {
    let mut ptr = pointer(16, b"abcdefghij");

    let PeekState::Request(peeked) = ptr.nextby(3).expect("nextby") else {
        panic!("expected data from nextby");
    };
    assert_eq!(peeked, b"abc");

    // Peeked bytes are not consumed yet, so a read must still return them.
    let mut buf = [0u8; 5];
    let n = ptr.read_size(&mut buf).unwrap();
    println!("read after nextby -> {:?}", std::str::from_utf8(&buf[..n]));
    assert_eq!(&buf[..n], b"abcde");
}

#[test]
fn read_shorter_than_the_peek_leaves_the_rest_unconsumed() {
    let mut ptr = pointer(16, b"abcdefghij");

    assert!(matches!(
        ptr.nextby(6).unwrap(),
        PeekState::Request(b"abcdef")
    ));

    let mut buf = [0u8; 2];
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 2);
    assert_eq!(&buf, b"ab");

    // Only the two returned bytes were consumed; the peek cursor is untouched.
    assert_eq!(ptr.data_cursor(), 2);
    assert_eq!(ptr.peek_cursor(), 6);
    assert_eq!(ptr.consume().unwrap(), b"cdef");

    let mut rest = [0u8; 8];
    let n = ptr.read_size(&mut rest).unwrap();
    assert_eq!(&rest[..n], b"ghij");
}

#[test]
fn read_after_forward_by_returns_the_peeked_bytes() {
    let mut ptr = pointer(16, b"abcdefghij");
    ptr.fill_up().expect("fill");
    assert_eq!(ptr.forward_by(4).unwrap(), PeekState::Continue);

    let mut buf = [0u8; 10];
    let n = ptr.read_size(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"abcdefghij");
}

#[test]
fn read_all_includes_bytes_only_peeked_at() {
    let mut ptr = pointer(4, b"abcdefghij");

    let mut first = [0u8; 2];
    assert_eq!(ptr.read_size(&mut first).unwrap(), 2);
    assert!(matches!(ptr.nextby(3).unwrap(), PeekState::Request(b"cde")));

    let mut all = Vec::new();
    let n = ptr.read_all(&mut all, None).unwrap();
    println!("read_all -> {:?}", std::str::from_utf8(&all));
    assert_eq!(n, 8);
    assert_eq!(all, b"cdefghij");

    let mut buf = [0u8; 4];
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 0);
}

#[test]
fn peekable_peek_does_not_consume() {
    let mut ptr = pointer(16, b"abcdefghij");

    let mut peeked = [0u8; 4];
    assert_eq!(PeekableReadStream::peek(&mut ptr, &mut peeked).unwrap(), 4);
    assert_eq!(&peeked, b"abcd");
    assert_eq!(ptr.peek_cursor(), 0, "peek must not move the peek cursor");

    let mut buf = [0u8; 6];
    assert_eq!(ptr.read(&mut buf).unwrap(), 6);
    assert_eq!(&buf, b"abcdef");
}

#[test]
fn peekable_peek_at_eof_reports_end_of_stream() {
    let mut ptr = pointer(16, b"ab");
    let mut buf = [0u8; 2];
    assert_eq!(ptr.read(&mut buf).unwrap(), 2);

    let mut peeked = [0u8; 4];
    assert!(matches!(
        PeekableReadStream::peek(&mut ptr, &mut peeked),
        Err(foundation_core::io::ioutils::PeekError::EndOfStream)
    ));
}

#[test]
fn reader_error_with_nothing_buffered_is_returned() {
    let reader = Scripted::new(vec![Err(io::ErrorKind::WouldBlock.into())]);
    let mut ptr = ByteBufferPointer::from_reader(8, reader);

    let mut buf = [0u8; 4];
    let err = ptr.read_size(&mut buf).expect_err("stall with no data");
    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn reader_error_after_some_bytes_returns_the_bytes_first() {
    let reader = Scripted::new(vec![
        Ok(b"abc".to_vec()),
        Err(io::ErrorKind::WouldBlock.into()),
        Err(io::ErrorKind::WouldBlock.into()),
        Ok(b"def".to_vec()),
    ]);
    let mut ptr = ByteBufferPointer::from_reader(8, reader);

    let mut buf = [0u8; 6];
    assert_eq!(ptr.read_size(&mut buf).unwrap(), 3);
    assert_eq!(&buf[..3], b"abc");

    let err = ptr.read_size(&mut buf).expect_err("still stalled");
    assert_eq!(err.kind(), io::ErrorKind::WouldBlock);

    assert_eq!(ptr.read_size(&mut buf).unwrap(), 3);
    assert_eq!(&buf[..3], b"def");
}

#[test]
fn shared_stream_reads_are_consecutive_across_refills() {
    let data = sample(300);
    let mut stream = SharedByteBufferStream::rwrite_with_capacity(16, Cursor::new(data.clone()));

    let mut collected = Vec::new();
    let mut buf = [0u8; 9];
    loop {
        let n = stream.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        collected.extend_from_slice(&buf[..n]);
    }
    assert_eq!(collected, data);
}

#[test]
fn shared_stream_read_exact_after_partial_read() {
    let mut stream =
        SharedByteBufferStream::rwrite_with_capacity(4, Cursor::new(b"0123456789".to_vec()));

    let mut head = [0u8; 3];
    assert_eq!(stream.read(&mut head).unwrap(), 3);
    assert_eq!(&head, b"012");

    let mut body = [0u8; 5];
    stream.read_exact(&mut body).unwrap();
    assert_eq!(&body, b"34567");

    let mut tail = Vec::new();
    stream.read_to_end(&mut tail).unwrap();
    assert_eq!(tail, b"89");
}
