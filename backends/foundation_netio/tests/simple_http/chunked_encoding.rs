//! Integration tests for HTTP chunked transfer encoding.
//!
//! These tests use raw TCP-captured data to verify the chunk parser
//! handles real-world server responses correctly.

#![allow(
    clippy::naive_bytecount,
    clippy::uninlined_format_args,
    clippy::byte_char_slices
)]

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::shared::http::{ChunkedData, SimpleHeaders, SimpleHttpChunkIterator};
use std::io::Cursor;
use tracing_test::traced_test;

/// The JSON payload the GCP-style fixture below carries, CRs and all.
///
/// It contains a stray CR inside a string value (`"requi\rred"`) — the thing
/// the GCP Discovery API was observed sending. That CR is *content*: the
/// transport must hand it over untouched, and it is the caller parsing the JSON
/// that has to decide what to do about it.
const GCP_STYLE_PAYLOAD: &[u8] =
    b"{\n \"kind\": \"discovery\",\n \"requi\rred\": \"value\"\n \"enumDescriptions\":\"test\"\n";

/// Builds a well-formed chunked body carrying `payload` split into two chunks.
///
/// The previous hand-written fixture folded each chunk's trailing CRLF *into*
/// its declared size and left out the inter-chunk delimiter, so its expected
/// output ended with the `0` of the terminator — parser bugs frozen as
/// "expected". This generates correct framing: `<hex size>CRLF <data> CRLF`,
/// terminated by `0CRLFCRLF`.
fn chunked_body(payload: &[u8]) -> Vec<u8> {
    let (first, second) = payload.split_at(payload.len() / 2);
    let mut raw = Vec::new();
    for chunk in [first, second] {
        raw.extend(format!("{:x}\r\n", chunk.len()).as_bytes());
        raw.extend_from_slice(chunk);
        raw.extend_from_slice(b"\r\n");
    }
    raw.extend_from_slice(b"0\r\n\r\n");
    raw
}

/// Integration test: a GCP-style response with a CR inside JSON content.
///
/// The parser used to strip every CR from chunk data so this JSON would parse.
/// That corrupted all binary chunked bodies — Docker's log frames put the
/// payload length in the header, so a 13-byte log line carries a literal 0x0D
/// and stripping it desynchronised the stream. Chunk data is opaque octets;
/// the size field says how many bytes to take, and CRLF only frames them.
#[test]
fn test_gcp_style_cr_in_json_is_preserved() {
    let cursor = Cursor::new(chunked_body(GCP_STYLE_PAYLOAD));
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    assert_eq!(
        &collected_bytes, GCP_STYLE_PAYLOAD,
        "the body must be reassembled byte-for-byte across chunk boundaries"
    );

    let cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(cr_count, 1, "the CR inside the JSON string is content");

    // And the terminator's `0` must not leak into the body.
    assert!(
        !collected_bytes.ends_with(b"0"),
        "the final chunk marker is framing, not data"
    );
}

/// Test: content with no CRs is unaffected (a plain round-trip check).
#[test]
fn test_clean_content_round_trips() {
    let chunk_data = b"{\"status\": \"ok\", \"count\": 42}";

    let mut raw_response = Vec::new();
    raw_response.extend(format!("{:x}\r\n", chunk_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk_data);
    raw_response.extend_from_slice(b"\r\n");
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    // Verify content is exactly as expected (no modification)
    assert_eq!(
        &collected_bytes, chunk_data,
        "Clean content should pass through unchanged"
    );
}

/// Test: Multi-chunk response with CR at various positions.
#[test]
fn test_cr_in_chunk_data_preserved_at_various_positions() {
    // Build chunks with CR at start, middle, and end of data
    let chunk1: &[u8] = b"\rstart"; // CR at start
    let chunk2: &[u8] = b"mid\rdle"; // CR in middle
    let chunk3: &[u8] = b"end\r"; // CR at end

    let mut raw_response = Vec::new();

    for chunk in &[chunk1, chunk2, chunk3] {
        raw_response.extend(format!("{:x}\r\n", chunk.len()).as_bytes());
        raw_response.extend_from_slice(chunk);
        raw_response.extend_from_slice(b"\r\n");
    }

    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    // Every CR was inside chunk data, so every CR must survive — including the
    // one at the very end of a chunk, right against the framing delimiter.
    let cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(cr_count, 3, "CRs in chunk data are content and must be kept");

    let expected = b"\rstartmid\rdleend\r";
    assert_eq!(
        &collected_bytes, expected,
        "chunk data must round-trip byte-for-byte"
    );
}

/// Integration test using binary fixture files captured from real HTTP responses.
///
/// This test loads pre-captured chunked encoding data from binary files
/// and verifies the parser correctly handles the response.
///
/// Fixture files:
/// - `gcp_chunked_fixture.bin` - Raw chunked HTTP body (as received over TCP)
/// - `gcp_chunked_expected.bin` - Expected output after parsing (CR stripped)
///
/// These fixtures simulate GCP Discovery API responses that contain stray
/// CR bytes in JSON string values.
#[test]
fn test_gcp_chunked_fixture_from_file() {
    // Load fixture data from binary file
    let fixture_bytes = include_bytes!("gcp_chunked_fixture.bin");
    let expected_bytes = include_bytes!("gcp_chunked_expected.bin");

    // Parse through SimpleHttpChunkIterator
    let cursor = Cursor::new(fixture_bytes);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    // The expected fixture is the exact concatenation of the chunk payloads, as
    // declared by each chunk's size field — CRs included. (It previously held the
    // CR-stripped bytes, 162 of the 171 the daemon actually sent.)
    assert_eq!(
        &collected_bytes, expected_bytes,
        "the captured body must be reassembled byte-for-byte"
    );

    // The capture carries stray CRs inside JSON strings; they are content and
    // survive. Cleaning them up is the JSON caller's business, not the
    // transport's.
    let cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(cr_count, 9, "every CR in the captured payload is preserved");

    let output_str = String::from_utf8_lossy(&collected_bytes);
    assert!(output_str.starts_with('{'), "should be the JSON object");
    assert!(
        output_str.contains("\"requi\rred\""),
        "the stray CR the server sent is still exactly where it was"
    );
}

/// Integration test using real captured GCP Discovery API response.
///
/// This test loads a raw chunked HTTP response captured from the actual
/// GCP Discovery API endpoint and verifies our parser handles it correctly.
///
/// Fixture: `gcp_real_chunked_response.bin` - 5.8MB raw chunked response body
/// from <https://www.googleapis.com/discovery/v1/apis/compute/v1/rest>
///
/// The captured response contains ~384 CR bytes embedded in JSON content
/// (not just framing) that must be stripped for valid JSON parsing.
#[test]
#[traced_test]
fn test_real_gcp_captured_response() {
    // Load the real captured GCP response (body only, with chunked encoding)
    let fixture_bytes = include_bytes!("gcp_real_chunked_response.bin");

    // Verify fixture contains CR bytes (proves this is real GCP data with issues)
    let fixture_cr_count = fixture_bytes.iter().filter(|&&b| b == b'\r').count();
    assert!(
        fixture_cr_count > 100,
        "Fixture should contain many CR bytes. Found: {}",
        fixture_cr_count
    );

    // Parse through SimpleHttpChunkIterator
    let cursor = Cursor::new(fixture_bytes);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();
    let mut chunk_count = 0;

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
                chunk_count += 1;
                tracing::info!(
                    "Chunk {}: collected {} bytes (total: {})",
                    chunk_count,
                    data.len(),
                    collected_bytes.len()
                );
            }
            Ok(ChunkedData::DataEnded) => {
                tracing::info!(
                    "DataEnd after {} chunks, {} bytes",
                    chunk_count,
                    collected_bytes.len()
                );
                break;
            }
            Ok(ChunkedData::Trailers(_)) => {
                tracing::info!("Trailers received");
            }
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    tracing::info!(
        "Final: {} chunks, {} bytes",
        chunk_count,
        collected_bytes.len()
    );

    // Verify all CR bytes are stripped from output
    let output_cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(
        output_cr_count, 0,
        "All CR bytes should be stripped from real GCP response. Found: {output_cr_count}"
    );

    // Verify output is valid JSON structure
    let output_str = String::from_utf8_lossy(&collected_bytes);
    assert!(
        output_str.starts_with('{'),
        "Should be valid JSON object starting with {{"
    );

    // Verify key GCP fields are present and correctly parsed
    assert!(
        output_str.contains("\"name\": \"compute\""),
        "Should contain compute API name"
    );
    assert!(
        output_str.contains("\"description\": \"Creates and runs virtual machines"),
        "Should contain compute API description"
    );

    // Verify size is reasonable (should be ~5.8MB minus CR bytes)
    assert!(
        collected_bytes.len() > 5_000_000,
        "Output should be ~5.8MB, got: {}",
        collected_bytes.len()
    );
}

/// Test: Non-standard chunked encoding with LF-only line endings (\n instead of \r\n).
///
/// Some servers may send chunked responses with LF-only line endings instead of
/// the standard CRLF. This test verifies the parser handles such non-standard
/// but functional responses correctly.
#[test]
fn test_lf_only_line_endings() {
    // Build a chunked response with LF-only line endings (no CR bytes)
    let chunk1: &[u8] = b"Hello, ";
    let chunk2: &[u8] = b"World!";

    let mut raw_response = Vec::new();

    // Chunk 1 with LF-only framing
    raw_response.extend(format!("{:x}\n", chunk1.len()).as_bytes());
    raw_response.extend_from_slice(chunk1);
    raw_response.extend_from_slice(b"\n");

    // Chunk 2 with LF-only framing
    raw_response.extend(format!("{:x}\n", chunk2.len()).as_bytes());
    raw_response.extend_from_slice(chunk2);
    raw_response.extend_from_slice(b"\n");

    // Final chunk with LF-only framing
    raw_response.extend_from_slice(b"0\n\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    // Verify content is correct
    let expected = b"Hello, World!";
    assert_eq!(
        &collected_bytes, expected,
        "Content should be correctly parsed with LF-only line endings"
    );
}

/// Test: Mixed line endings - some chunks with CRLF, some with LF-only.
///
/// This test verifies the parser handles inconsistent line endings across
/// different chunks within the same response.
#[test]
fn test_mixed_line_endings() {
    let chunk1: &[u8] = b"Chunk1-";
    let chunk2: &[u8] = b"Chunk2-";
    let chunk3: &[u8] = b"Chunk3";

    let mut raw_response = Vec::new();

    // Chunk 1 with standard CRLF framing
    raw_response.extend(format!("{:x}\r\n", chunk1.len()).as_bytes());
    raw_response.extend_from_slice(chunk1);
    raw_response.extend_from_slice(b"\r\n");

    // Chunk 2 with LF-only framing
    raw_response.extend(format!("{:x}\n", chunk2.len()).as_bytes());
    raw_response.extend_from_slice(chunk2);
    raw_response.extend_from_slice(b"\n");

    // Chunk 3 with standard CRLF framing
    raw_response.extend(format!("{:x}\r\n", chunk3.len()).as_bytes());
    raw_response.extend_from_slice(chunk3);
    raw_response.extend_from_slice(b"\r\n");

    // Final chunk with CRLF
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    // Verify content is correct
    let expected = b"Chunk1-Chunk2-Chunk3";
    assert_eq!(
        &collected_bytes, expected,
        "Content should be correctly parsed with mixed line endings"
    );
}

/// Test: with LF-only framing, chunk data that itself begins with LF survives.
///
/// This replaces an earlier `test_double_lf_line_endings`, which framed chunks as
/// `<size>\n\n<data>\n\n` and asserted the parser skipped both LFs. That framing
/// is not something any server sends (Docker and the captured GCP response both
/// use CRLF), and supporting it is *unavoidably* in conflict with binary
/// correctness: given LF-only framing, `<size>\n` followed by data starting with
/// `\n` is byte-identical to `<size>\n\n` followed by data. The parser resolves
/// that ambiguity in favour of the data — it consumes exactly one terminator,
/// because the chunk-size already says precisely how many bytes of data follow.
#[test]
fn test_lf_framing_keeps_leading_lf_in_data() {
    let chunk1: &[u8] = b"\nData part 1, ";
    let chunk2: &[u8] = b"Data part 2.";

    let mut raw_response = Vec::new();

    // LF-only framing: one LF after the size, one after the data.
    raw_response.extend(format!("{:x}\n", chunk1.len()).as_bytes());
    raw_response.extend_from_slice(chunk1);
    raw_response.extend_from_slice(b"\n");

    raw_response.extend(format!("{:x}\n", chunk2.len()).as_bytes());
    raw_response.extend_from_slice(chunk2);
    raw_response.extend_from_slice(b"\n");

    raw_response.extend_from_slice(b"0\n\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {e}"),
        }
    }

    // The leading LF belongs to chunk 1's data — the size field counted it.
    let expected = b"\nData part 1, Data part 2.";
    assert_eq!(
        &collected_bytes, expected,
        "LF-only framing must not eat an LF that is part of the chunk data"
    );
}
