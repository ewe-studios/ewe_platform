//! A peer reset on an unframed event-stream or line body must reach the caller
//! as an error, never as a clean end of stream.
//!
//! WHY: the reconnect decision keys on the close reason. A body that reports a
//! reset as end of stream closes as `Eof`, so the reconnecting task does not
//! retry (issue #1252). The content decoder added in #1220 put an extra
//! buffered reader above the transport. That reader asked the inner stream for
//! a full buffer, so the inner stream read the reset that arrived with the
//! last data bytes, swallowed it, and reported a clean end on the next read.
//!
//! WHAT: feed `SimpleHttpBody::extract` a reader that plays a reset socket:
//! data first, then one `ConnectionReset`, then end of stream. The body
//! iterator must yield the parsed item and then an error. With no reset (end of
//! stream after the data) it must still end cleanly.

use std::io::{self, Cursor, Read};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::shared::http::{
    Body, BodyExtractor, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleHttpBody,
};

/// A reader that plays the sequence a peer reset produces on a real socket:
/// the data already in flight first, the reset error once, then end of stream
/// once the drained socket is read again.
struct ResetAfterData {
    data: Vec<u8>,
    offset: usize,
    reset_reported: bool,
}

impl ResetAfterData {
    fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            offset: 0,
            reset_reported: false,
        }
    }
}

impl Read for ResetAfterData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.offset < self.data.len() {
            let n = (self.data.len() - self.offset).min(buf.len());
            buf[..n].copy_from_slice(&self.data[self.offset..self.offset + n]);
            self.offset += n;
            return Ok(n);
        }
        if !self.reset_reported {
            self.reset_reported = true;
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "peer reset the connection",
            ));
        }
        Ok(0)
    }
}

fn sse_headers() -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["text/event-stream".to_string()],
    );
    headers
}

/// The reset is reported once, after the last data bytes, exactly as a real
/// socket reports it. The event-stream parser must surface it as an error.
#[test]
fn sse_reset_after_data_is_an_error_not_end_of_stream() {
    let stream = SharedByteBufferStream::rwrite(ResetAfterData::new(b"data: hello\n\n"));
    let body = SimpleHttpBody::default()
        .extract(Body::SseBody(sse_headers()), stream)
        .expect("the event-stream body must be extracted");
    let SendSafeBody::SseStream(Some(mut iterator)) = body else {
        panic!("expected an SSE stream body");
    };

    let first = iterator.next().expect("the event before the reset");
    assert!(first.is_ok(), "the event before the reset must parse");

    match iterator.next() {
        Some(Err(_)) => {}
        Some(Ok(_)) => panic!("the reset must not parse as another event"),
        None => panic!("the reset must surface as an error, not a clean end of stream"),
    }
}

/// A line-delimited body goes through the same transport chain and must keep
/// the same error surface.
#[test]
fn line_feed_reset_after_data_is_an_error_not_end_of_stream() {
    // A line ends at CRLF, and the parser then eats the CR/LF/space run after
    // it. With no byte past that run, the line is complete only at end of
    // input, so the next pull from the transport is the reset itself.
    let stream = SharedByteBufferStream::rwrite(ResetAfterData::new(b"hello\r\nworld\r\n"));
    let body = SimpleHttpBody::default()
        .extract(Body::LineFeedBody(SimpleHeaders::new()), stream)
        .expect("the line-delimited body must be extracted");
    let SendSafeBody::LineFeedStream(Some(mut iterator)) = body else {
        panic!("expected a line-feed stream body");
    };

    assert!(
        iterator.next().expect("first line").is_ok(),
        "the first line before the reset must parse"
    );

    match iterator.next() {
        Some(Err(_)) => {}
        Some(Ok(_)) => panic!("the reset must not parse as another line"),
        None => panic!("the reset must surface as an error, not a clean end of stream"),
    }
}

/// A line body whose final line is a single byte past a CRLF tail must not
/// panic. `nextby2(2)` comes back short at end of input; moving the peek
/// cursor back by the requested two bytes subtracted below zero.
#[test]
fn line_feed_one_byte_tail_does_not_panic() {
    let stream = SharedByteBufferStream::rwrite(Cursor::new(b"hello\r\nworld\r\nx".to_vec()));
    let body = SimpleHttpBody::default()
        .extract(Body::LineFeedBody(SimpleHeaders::new()), stream)
        .expect("the line-delimited body must be extracted");
    let SendSafeBody::LineFeedStream(Some(iterator)) = body else {
        panic!("expected a line-feed stream body");
    };

    let lines: Vec<Result<_, _>> = iterator.collect();
    assert!(
        lines.len() >= 2 && lines[0].is_ok() && lines[1].is_ok(),
        "the two complete lines must parse before the one-byte tail"
    );
}

/// A body that really ends after its data still ends cleanly: the guard above
/// must not turn a clean end of stream into an error.
#[test]
fn sse_clean_end_after_data_still_ends_the_stream() {
    let stream = SharedByteBufferStream::rwrite(Cursor::new(b"data: hello\n\n".to_vec()));
    let body = SimpleHttpBody::default()
        .extract(Body::SseBody(sse_headers()), stream)
        .expect("the event-stream body must be extracted");
    let SendSafeBody::SseStream(Some(mut iterator)) = body else {
        panic!("expected an SSE stream body");
    };

    assert!(
        iterator.next().expect("the event").is_ok(),
        "the event must parse"
    );
    assert!(
        iterator.next().is_none(),
        "a clean end of stream after the data must end the stream"
    );
}
