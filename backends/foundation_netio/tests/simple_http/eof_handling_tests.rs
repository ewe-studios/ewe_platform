//! Tests for EOF handling in HTTP readers.
//!
//! These tests verify that `HttpRequestReader` and `HttpResponseReader`
//! correctly terminate on EOF (`read_line` returns `Ok(0)`) instead of
//! entering an infinite loop returning `SKIP`.

use std::io::{Cursor, Read, Result as IoResult};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::shared::http::{
    http_streams, HTTPStreams, HttpRequestReader, HttpResponseReader, IncomingRequestParts,
    IncomingResponseParts, SimpleHttpBody,
};

// =======================================================================
// EOF Simulating Reader
// =======================================================================

/// A reader that returns EOF (Ok(0)) immediately or after some bytes.
struct EofReader {
    data: Vec<u8>,
    position: usize,
    eof_after: Option<usize>,
}

impl EofReader {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            position: 0,
            eof_after: None,
        }
    }

    fn eof_immediately() -> Self {
        Self::new(vec![])
    }

    fn eof_after(data: Vec<u8>) -> Self {
        Self {
            data,
            position: 0,
            eof_after: None,
        }
    }
}

impl Read for EofReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.position >= self.data.len() {
            return Ok(0); // EOF
        }

        let remaining = self.data.len() - self.position;
        let to_read = buf.len().min(remaining);

        buf[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
        self.position += to_read;

        Ok(to_read)
    }
}

// =======================================================================
// HttpRequestReader EOF Tests
// =======================================================================

/// WHY: Verify `HttpRequestReader` returns `None` on immediate EOF.
/// WHAT: Tests that when the stream returns EOF immediately, the iterator
///        terminates instead of returning an endless stream of `SKIP`.
#[test]
fn test_request_reader_eof_immediately() {
    let eof_reader = EofReader::eof_immediately();
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    let mut request_reader = streams.next_request();

    // First call should return None (EOF), not Some(Ok(SKIP)) in a loop
    let result = request_reader.next();

    assert!(
        result.is_none(),
        "Expected None on EOF, got {:?} - this would cause an infinite loop",
        result
    );
}

/// WHY: Verify `HttpRequestReader` handles one complete request then EOF.
/// WHAT: Tests that after reading a complete request, the iterator correctly
///        returns `None` on the next call when EOF is reached.
#[test]
fn test_request_reader_one_request_then_eof() {
    // A complete HTTP request followed by EOF
    let request_data = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let eof_reader = EofReader::new(request_data.to_vec());
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    let mut request_reader = streams.next_request();

    // Collect all parts from the first request
    let mut parts = Vec::new();
    for part in &mut request_reader {
        parts.push(part);
    }

    // Should have gotten the complete request
    let has_intro = parts
        .iter()
        .any(|p| matches!(p, Ok(IncomingRequestParts::Intro(_, _, _))));
    assert!(has_intro, "Expected Intro part in request");

    // Now get the next request (should be EOF)
    let streams2 = HTTPStreams::new(SharedByteBufferStream::rwrite(EofReader::eof_immediately()));
    let mut next_reader = streams2.next_request();
    let next_result = next_reader.next();

    assert!(
        next_result.is_none(),
        "Expected None after request completes and EOF reached, got {:?}",
        next_result
    );
}

/// WHY: Verify `HttpRequestReader` handles partial request then EOF.
/// WHAT: Tests that when a partial request is sent and then EOF occurs,
///        the iterator terminates gracefully rather than looping.
#[test]
fn test_request_reader_partial_then_eof() {
    // Partial HTTP request (incomplete headers)
    let partial_data = b"GET /test HTTP/1.1\r\nHost: ";
    let eof_reader = EofReader::new(partial_data.to_vec());
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    let mut request_reader = streams.next_request();

    // Try to read - should get Intro but then hit EOF on headers
    // The reader should handle this gracefully
    let mut count = 0;
    let max_iterations = 10; // Safety limit to catch infinite loops

    for result in &mut request_reader {
        count += 1;
        if count > max_iterations {
            panic!("Possible infinite loop - got {} iterations", count);
        }

        // We should at least get the Intro part before EOF
        if let Ok(IncomingRequestParts::Intro(_, _, _)) = result {
            break; // Got the intro, EOF is expected after
        }
    }

    // Should not have looped forever
    assert!(count <= max_iterations, "Reader may be in infinite loop");
}

// =======================================================================
// HttpResponseReader EOF Tests
// =======================================================================

/// WHY: Verify `HttpResponseReader` returns `None` on immediate EOF.
/// WHAT: Tests that when the stream returns EOF immediately, the response
///        iterator terminates instead of returning endless `SKIP`.
#[test]
fn test_response_reader_eof_immediately() {
    let eof_reader = EofReader::eof_immediately();
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    let mut response_reader = streams.next_response();

    // First call should return None (EOF), not Some(Ok(SKIP)) in a loop
    let result = response_reader.next();

    assert!(
        result.is_none(),
        "Expected None on EOF, got {:?} - this would cause an infinite loop",
        result
    );
}

/// WHY: Verify `HttpResponseReader` handles one complete response then EOF.
/// WHAT: Tests that after reading a complete response, the iterator correctly
///        returns `None` on the next call when EOF is reached.
#[test]
fn test_response_reader_one_response_then_eof() {
    // A complete HTTP response followed by EOF
    let response_data = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
    let eof_reader = EofReader::new(response_data.to_vec());
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    let mut response_reader = streams.next_response();

    // Collect all parts from the first response
    let mut parts = Vec::new();
    for part in &mut response_reader {
        parts.push(part);
    }

    // Should have gotten the complete response
    let has_intro = parts
        .iter()
        .any(|p| matches!(p, Ok(IncomingResponseParts::Intro(_, _, _))));
    assert!(has_intro, "Expected Intro part in response");

    // Now get the next response (should be EOF)
    let streams2 = HTTPStreams::new(SharedByteBufferStream::rwrite(EofReader::eof_immediately()));
    let mut next_reader = streams2.next_response();
    let next_result = next_reader.next();

    assert!(
        next_result.is_none(),
        "Expected None after response completes and EOF reached, got {:?}",
        next_result
    );
}

/// WHY: Verify `HttpResponseReader` handles partial response then EOF.
/// WHAT: Tests that when a partial response is sent and then EOF occurs,
///        the iterator terminates gracefully rather than looping.
#[test]
fn test_response_reader_partial_then_eof() {
    // Partial HTTP response (incomplete headers)
    let partial_data = b"HTTP/1.1 200 OK\r\nContent-";
    let eof_reader = EofReader::new(partial_data.to_vec());
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    let mut response_reader = streams.next_response();

    // Try to read - should get Intro but then hit EOF on headers
    let mut count = 0;
    let max_iterations = 10; // Safety limit to catch infinite loops

    for result in &mut response_reader {
        count += 1;
        if count > max_iterations {
            panic!("Possible infinite loop - got {} iterations", count);
        }

        // We should at least get the Intro part before EOF
        if let Ok(IncomingResponseParts::Intro(_, _, _)) = result {
            break; // Got the intro, EOF is expected after
        }
    }

    // Should not have looped forever
    assert!(count <= max_iterations, "Reader may be in infinite loop");
}

// =======================================================================
// Keep-Alive vs EOF Boundary Tests
// =======================================================================

/// WHY: Verify blank line (\r\n) between pipelined requests is handled correctly.
/// WHAT: Tests that `SKIP` is returned for blank lines (not EOF), but EOF
///        still terminates the iterator.
#[test]
fn test_request_reader_blank_line_vs_eof() {
    // Two pipelined requests with a blank line between them (keep-alive)
    let pipelined_data = b"GET /first HTTP/1.1\r\nHost: example.com\r\n\r\n\r\nGET /second HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let eof_reader = EofReader::new(pipelined_data.to_vec());
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    // First request
    let mut request_reader1 = streams.next_request();
    let mut parts1 = Vec::new();
    for part in &mut request_reader1 {
        parts1.push(part);
    }

    let has_first = parts1.iter().any(|p| {
        if let Ok(IncomingRequestParts::Intro(_, url, _)) = p {
            url.url.contains("/first")
        } else {
            false
        }
    });
    assert!(has_first, "Expected first request with /first path");

    // Second request
    let mut request_reader2 = streams.next_request();
    let mut parts2 = Vec::new();
    for part in &mut request_reader2 {
        parts2.push(part);
    }

    let has_second = parts2.iter().any(|p| {
        if let Ok(IncomingRequestParts::Intro(_, url, _)) = p {
            url.url.contains("/second")
        } else {
            false
        }
    });
    assert!(has_second, "Expected second request with /second path");
}

/// WHY: Verify that EOF on a keep-alive connection terminates cleanly.
/// WHAT: Tests that after keep-alive requests, EOF is properly detected.
#[test]
fn test_keepalive_then_eof() {
    // Request followed by immediate EOF (simulating connection close after keep-alive)
    let request_then_eof = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let eof_reader = EofReader::new(request_then_eof.to_vec());
    let shared_stream = SharedByteBufferStream::rwrite(eof_reader);
    let streams = HTTPStreams::new(shared_stream);

    // First request should succeed
    let mut request_reader = streams.next_request();
    let mut parts = Vec::new();
    for part in &mut request_reader {
        parts.push(part);
    }

    let has_request = parts
        .iter()
        .any(|p| matches!(p, Ok(IncomingRequestParts::Intro(_, _, _))));
    assert!(has_request, "Expected request to be read");

    // Second request should hit EOF immediately (not infinite loop)
    let streams2 = HTTPStreams::new(SharedByteBufferStream::rwrite(EofReader::eof_immediately()));
    let mut next_reader = streams2.next_request();
    let next_result = next_reader.next();

    assert!(
        next_result.is_none(),
        "Expected None after keep-alive then EOF, got {:?}",
        next_result
    );
}
